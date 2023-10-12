use super::{
    instruction_builder::InstructionBuilder, name_generator::NameGenerator, typed_expression::*,
};
use crate::{
    ir::*,
    types::{self, Type},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct ModuleBuilder {
    pub name_generator: Rc<RefCell<NameGenerator>>,
    pub variable_declarations: RefCell<Vec<VariableDeclaration>>,
    pub function_declarations: RefCell<Vec<FunctionDeclaration>>,
    pub variable_definitions: RefCell<Vec<VariableDefinition>>,
    pub function_definitions: RefCell<Vec<FunctionDefinition>>,
}

impl ModuleBuilder {
    pub fn new() -> Self {
        Self {
            name_generator: Rc::new(NameGenerator::new("_fmm_").into()),
            variable_declarations: Default::default(),
            function_declarations: Default::default(),
            variable_definitions: Default::default(),
            function_definitions: Default::default(),
        }
    }

    pub fn into_module(self) -> Module {
        Module::new(
            self.variable_declarations.into_inner(),
            self.function_declarations.into_inner(),
            self.variable_definitions.into_inner(),
            self.function_definitions.into_inner(),
        )
    }

    pub fn declare_variable(
        &self,
        name: impl Into<String>,
        type_: impl Into<Type>,
    ) -> TypedExpression {
        let name = name.into();
        let type_ = type_.into();

        self.variable_declarations
            .borrow_mut()
            .push(VariableDeclaration::new(&name, type_.clone()));

        TypedExpression::new(Variable::new(name), types::Pointer::new(type_))
    }

    pub fn declare_function(
        &self,
        name: impl Into<String>,
        type_: types::Function,
    ) -> TypedExpression {
        let name = name.into();

        self.function_declarations
            .borrow_mut()
            .push(FunctionDeclaration::new(&name, type_.clone()));

        TypedExpression::new(Variable::new(name), type_)
    }

    pub fn define_variable(
        &self,
        name: impl Into<String>,
        body: impl Into<TypedExpression>,
        options: VariableDefinitionOptions,
    ) -> TypedExpression {
        let name = name.into();
        let body = body.into();

        self.variable_definitions
            .borrow_mut()
            .push(VariableDefinition::new(
                &name,
                body.expression().clone(),
                body.type_().clone(),
                options,
            ));

        TypedExpression::new(
            Variable::new(name),
            types::Pointer::new(body.type_().clone()),
        )
    }

    pub fn define_anonymous_variable(
        &self,
        body: impl Into<TypedExpression>,
        options: VariableDefinitionOptions,
    ) -> TypedExpression {
        self.define_variable(
            self.generate_name(),
            body,
            options.set_linkage(Linkage::Internal),
        )
    }

    pub fn define_function<E>(
        &self,
        name: impl Into<String>,
        arguments: Vec<Argument>,
        result_type: impl Into<Type>,
        body: impl Fn(InstructionBuilder) -> Result<Block, E>,
        options: FunctionDefinitionOptions,
    ) -> Result<TypedExpression, E> {
        let name = name.into();
        let function_definition = FunctionDefinition::new(
            &name,
            arguments,
            result_type.into(),
            body(InstructionBuilder::new(self.name_generator.clone()))?,
            options,
        );
        let type_ = function_definition.type_();

        self.function_definitions
            .borrow_mut()
            .push(function_definition);

        Ok(TypedExpression::new(Variable::new(name), type_))
    }

    pub fn define_anonymous_function<E>(
        &self,
        origin_name: String,
        arguments: Vec<Argument>,
        result_type: impl Into<Type>,
        body: impl Fn(InstructionBuilder) -> Result<Block, E>,
        options: FunctionDefinitionOptions,
    ) -> Result<TypedExpression, E> {
        self.define_function(
            format!("{}_{}", self.generate_name(), origin_name),
            arguments,
            result_type,
            body,
            options.set_linkage(Linkage::Internal),
        )
    }

    pub fn generate_name(&self) -> String {
        self.name_generator.borrow_mut().generate()
    }
}

impl Default for ModuleBuilder {
    fn default() -> Self {
        Self::new()
    }
}
