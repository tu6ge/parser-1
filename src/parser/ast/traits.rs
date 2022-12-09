use crate::lexer::token::Span;
use crate::parser::ast::attributes::AttributeGroup;
use crate::parser::ast::constant::ClassishConstant;
use crate::parser::ast::functions::Method;
use crate::parser::ast::identifiers::Identifier;
use crate::parser::ast::modifiers::VisibilityModifier;
use crate::parser::ast::properties::Property;
use crate::parser::ast::properties::VariableProperty;

#[derive(Debug, PartialEq, Clone)]
pub struct Trait {
    pub start: Span,
    pub end: Span,
    pub name: Identifier,
    pub attributes: Vec<AttributeGroup>,
    pub members: Vec<TraitMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraitMember {
    Constant(ClassishConstant),
    TraitUsage(TraitUsage),
    Property(Property),
    VariableProperty(VariableProperty),
    Method(Method),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitUsage {
    pub traits: Vec<Identifier>,
    pub adaptations: Vec<TraitUsageAdaptation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraitUsageAdaptation {
    Alias {
        r#trait: Option<Identifier>,
        method: Identifier,
        alias: Identifier,
        visibility: Option<VisibilityModifier>,
    },
    Visibility {
        r#trait: Option<Identifier>,
        method: Identifier,
        visibility: VisibilityModifier,
    },
    Precedence {
        r#trait: Option<Identifier>,
        method: Identifier,
        insteadof: Vec<Identifier>,
    },
}