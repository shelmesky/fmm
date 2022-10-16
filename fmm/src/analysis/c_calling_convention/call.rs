use super::{context::Context, error::CCallingConventionError, type_};
use crate::{
    build::{InstructionBuilder, NameGenerator, TypedExpression},
    ir::*,
    types,
};
use std::rc::Rc;

pub fn transform_function_definition(
    context: &Context,
    definition: &FunctionDefinition,
) -> Result<FunctionDefinition, CCallingConventionError> {
    Ok(FunctionDefinition::new(
        definition.name(),
        definition.arguments().to_vec(),
        definition.result_type().clone(),
        transform_block(context, definition.body())?,
        definition.options().clone(),
    ))
}

fn transform_block(context: &Context, block: &Block) -> Result<Block, CCallingConventionError> {
    Ok(Block::new(
        block
            .instructions()
            .iter()
            .map(|instruction| transform_instruction(context, instruction))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect(),
        block.terminal_instruction().clone(),
    ))
}

fn transform_instruction(
    context: &Context,
    instruction: &Instruction,
) -> Result<Vec<Instruction>, CCallingConventionError> {
    Ok(match instruction {
        Instruction::Call(call)
            if call.type_().calling_convention() == types::CallingConvention::Target =>
        {
            let builder = InstructionBuilder::new(Rc::new(
                NameGenerator::new(format!("{}_c_", call.name())).into(),
            ));
            let original_function_type = call.type_();
            let function_type = type_::transform_function(context, original_function_type);
            let function = TypedExpression::new(call.function().clone(), function_type.clone());

            let mut arguments = vec![];

            for (argument, type_) in call
                .arguments()
                .iter()
                .zip(original_function_type.arguments())
            {
                let argument = TypedExpression::new(argument.clone(), type_.clone());

                if type_::is_memory_class(context, type_) {
                    let pointer = builder.allocate_stack(type_.clone());

                    builder.store(argument, pointer.clone());

                    arguments.push(pointer);
                } else {
                    arguments.push(argument);
                }
            }

            if type_::is_memory_class(context, original_function_type.result()) {
                let pointer = builder.allocate_stack(original_function_type.result().clone());

                builder.call(
                    function,
                    [pointer.clone()].into_iter().chain(arguments).collect(),
                )?;

                builder.add_instruction(Load::new(
                    original_function_type.result().clone(),
                    pointer.expression().clone(),
                    call.name(),
                ));
            } else {
                builder.add_instruction(Call::new(
                    function_type,
                    function.expression().clone(),
                    arguments
                        .into_iter()
                        .map(|argument| argument.expression().clone())
                        .collect(),
                    call.name(),
                ));
            }

            builder.into_instructions()
        }
        Instruction::If(if_) => vec![If::new(
            if_.type_().clone(),
            if_.condition().clone(),
            transform_block(context, if_.then())?,
            transform_block(context, if_.else_())?,
            if_.name(),
        )
        .into()],
        _ => vec![instruction.clone()],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::void_type;
    use pretty_assertions::assert_eq;

    const WORD_BYTES: usize = 8;

    #[test]
    fn transform_compatible() {
        let definition = FunctionDefinition::new(
            "f",
            vec![],
            types::Primitive::Integer64,
            Block::new(
                vec![Call::new(
                    types::Function::new(
                        vec![types::Primitive::Integer64.into()],
                        types::Primitive::Integer64,
                        types::CallingConvention::Target,
                    ),
                    Variable::new("g"),
                    vec![Undefined::new(types::Primitive::Integer64).into()],
                    "x",
                )
                .into()],
                Return::new(types::Primitive::Integer64, Variable::new("x")),
            ),
            FunctionDefinitionOptions::new()
                .set_calling_convention(types::CallingConvention::Target),
        );

        assert_eq!(
            transform_function_definition(&Context::new(WORD_BYTES), &definition),
            Ok(definition)
        );
    }

    #[test]
    fn transform_argument() {
        let record_type = types::Record::new(vec![
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
        ]);

        assert_eq!(
            transform_function_definition(
                &Context::new(WORD_BYTES),
                &FunctionDefinition::new(
                    "f",
                    vec![],
                    types::Primitive::Integer64,
                    Block::new(
                        vec![Call::new(
                            types::Function::new(
                                vec![record_type.clone().into()],
                                types::Primitive::Integer64,
                                types::CallingConvention::Target,
                            ),
                            Variable::new("g"),
                            vec![Undefined::new(record_type.clone()).into()],
                            "x",
                        )
                        .into()],
                        Return::new(types::Primitive::Integer64, Variable::new("x")),
                    ),
                    FunctionDefinitionOptions::new()
                        .set_calling_convention(types::CallingConvention::Target),
                )
            ),
            Ok(FunctionDefinition::new(
                "f",
                vec![],
                types::Primitive::Integer64,
                Block::new(
                    vec![
                        AllocateStack::new(record_type.clone(), "x_c_0").into(),
                        Store::new(
                            record_type.clone(),
                            Undefined::new(record_type.clone()),
                            Variable::new("x_c_0")
                        )
                        .into(),
                        Call::new(
                            types::Function::new(
                                vec![types::Pointer::new(record_type).into()],
                                types::Primitive::Integer64,
                                types::CallingConvention::Target,
                            ),
                            Variable::new("g"),
                            vec![Variable::new("x_c_0").into()],
                            "x",
                        )
                        .into()
                    ],
                    Return::new(types::Primitive::Integer64, Variable::new("x")),
                ),
                FunctionDefinitionOptions::new()
                    .set_calling_convention(types::CallingConvention::Target),
            ))
        );
    }

    #[test]
    fn transform_result() {
        let record_type = types::Record::new(vec![
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
        ]);

        assert_eq!(
            transform_function_definition(
                &Context::new(WORD_BYTES),
                &FunctionDefinition::new(
                    "f",
                    vec![],
                    types::Primitive::Integer64,
                    Block::new(
                        vec![
                            Call::new(
                                types::Function::new(
                                    vec![],
                                    record_type.clone(),
                                    types::CallingConvention::Target,
                                ),
                                Variable::new("f"),
                                vec![],
                                "x",
                            )
                            .into(),
                            DeconstructRecord::new(record_type.clone(), Variable::new("x"), 0, "y")
                                .into(),
                        ],
                        Return::new(types::Primitive::Integer64, Variable::new("y")),
                    ),
                    FunctionDefinitionOptions::new()
                        .set_calling_convention(types::CallingConvention::Target),
                )
            ),
            Ok(FunctionDefinition::new(
                "f",
                vec![],
                types::Primitive::Integer64,
                Block::new(
                    vec![
                        AllocateStack::new(record_type.clone(), "x_c_0").into(),
                        Call::new(
                            types::Function::new(
                                vec![types::Pointer::new(record_type.clone()).into()],
                                void_type(),
                                types::CallingConvention::Target
                            ),
                            Variable::new("f"),
                            vec![Variable::new("x_c_0").into()],
                            "x_c_1"
                        )
                        .into(),
                        Load::new(record_type.clone(), Variable::new("x_c_0"), "x").into(),
                        DeconstructRecord::new(record_type, Variable::new("x"), 0, "y").into(),
                    ],
                    Return::new(types::Primitive::Integer64, Variable::new("y")),
                ),
                FunctionDefinitionOptions::new()
                    .set_calling_convention(types::CallingConvention::Target),
            ))
        );
    }

    #[test]
    fn transform_result_with_argument() {
        let record_type = types::Record::new(vec![
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
        ]);

        assert_eq!(
            transform_function_definition(
                &Context::new(WORD_BYTES),
                &FunctionDefinition::new(
                    "f",
                    vec![],
                    types::Primitive::Integer64,
                    Block::new(
                        vec![
                            Call::new(
                                types::Function::new(
                                    vec![types::Primitive::PointerInteger.into()],
                                    record_type.clone(),
                                    types::CallingConvention::Target,
                                ),
                                Variable::new("f"),
                                vec![Primitive::PointerInteger(42).into()],
                                "x",
                            )
                            .into(),
                            DeconstructRecord::new(record_type.clone(), Variable::new("x"), 0, "y")
                                .into(),
                        ],
                        Return::new(types::Primitive::Integer64, Variable::new("y")),
                    ),
                    FunctionDefinitionOptions::new()
                        .set_calling_convention(types::CallingConvention::Target),
                )
            ),
            Ok(FunctionDefinition::new(
                "f",
                vec![],
                types::Primitive::Integer64,
                Block::new(
                    vec![
                        AllocateStack::new(record_type.clone(), "x_c_0").into(),
                        Call::new(
                            types::Function::new(
                                vec![
                                    types::Pointer::new(record_type.clone()).into(),
                                    types::Primitive::PointerInteger.into()
                                ],
                                void_type(),
                                types::CallingConvention::Target
                            ),
                            Variable::new("f"),
                            vec![
                                Variable::new("x_c_0").into(),
                                Primitive::PointerInteger(42).into()
                            ],
                            "x_c_1"
                        )
                        .into(),
                        Load::new(record_type.clone(), Variable::new("x_c_0"), "x").into(),
                        DeconstructRecord::new(record_type, Variable::new("x"), 0, "y").into(),
                    ],
                    Return::new(types::Primitive::Integer64, Variable::new("y")),
                ),
                FunctionDefinitionOptions::new()
                    .set_calling_convention(types::CallingConvention::Target),
            ))
        );
    }

    #[test]
    fn transform_in_nested_block() {
        let record_type = types::Record::new(vec![
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
            types::Primitive::Integer64.into(),
        ]);

        assert_eq!(
            transform_function_definition(
                &Context::new(WORD_BYTES),
                &FunctionDefinition::new(
                    "f",
                    vec![],
                    void_type(),
                    Block::new(
                        vec![If::new(
                            void_type(),
                            Primitive::Boolean(true),
                            Block::new(
                                vec![Call::new(
                                    types::Function::new(
                                        vec![record_type.clone().into()],
                                        void_type(),
                                        types::CallingConvention::Target,
                                    ),
                                    Variable::new("g"),
                                    vec![Undefined::new(record_type.clone()).into()],
                                    "x",
                                )
                                .into()],
                                Return::new(types::Primitive::Integer64, Variable::new("x")),
                            ),
                            Block::new(
                                vec![Call::new(
                                    types::Function::new(
                                        vec![record_type.clone().into()],
                                        void_type(),
                                        types::CallingConvention::Target,
                                    ),
                                    Variable::new("g"),
                                    vec![Undefined::new(record_type.clone()).into()],
                                    "y",
                                )
                                .into()],
                                Return::new(types::Primitive::Integer64, Variable::new("y")),
                            ),
                            ""
                        )
                        .into(),],
                        Return::new(void_type(), void_value()),
                    ),
                    FunctionDefinitionOptions::new()
                        .set_calling_convention(types::CallingConvention::Target),
                )
            ),
            Ok(FunctionDefinition::new(
                "f",
                vec![],
                void_type(),
                Block::new(
                    vec![If::new(
                        void_type(),
                        Primitive::Boolean(true),
                        Block::new(
                            vec![
                                AllocateStack::new(record_type.clone(), "x_c_0").into(),
                                Store::new(
                                    record_type.clone(),
                                    Undefined::new(record_type.clone()),
                                    Variable::new("x_c_0"),
                                )
                                .into(),
                                Call::new(
                                    types::Function::new(
                                        vec![types::Pointer::new(record_type.clone()).into()],
                                        void_type(),
                                        types::CallingConvention::Target,
                                    ),
                                    Variable::new("g"),
                                    vec![Variable::new("x_c_0").into()],
                                    "x",
                                )
                                .into(),
                            ],
                            Return::new(types::Primitive::Integer64, Variable::new("x")),
                        ),
                        Block::new(
                            vec![
                                AllocateStack::new(record_type.clone(), "y_c_0").into(),
                                Store::new(
                                    record_type.clone(),
                                    Undefined::new(record_type.clone()),
                                    Variable::new("y_c_0"),
                                )
                                .into(),
                                Call::new(
                                    types::Function::new(
                                        vec![types::Pointer::new(record_type).into()],
                                        void_type(),
                                        types::CallingConvention::Target,
                                    ),
                                    Variable::new("g"),
                                    vec![Variable::new("y_c_0").into()],
                                    "y",
                                )
                                .into(),
                            ],
                            Return::new(types::Primitive::Integer64, Variable::new("y")),
                        ),
                        ""
                    )
                    .into(),],
                    Return::new(void_type(), void_value()),
                ),
                FunctionDefinitionOptions::new()
                    .set_calling_convention(types::CallingConvention::Target),
            ))
        );
    }
}