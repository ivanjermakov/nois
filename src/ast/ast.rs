use regex::Regex;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub struct Block(pub Vec<AstPair<Statement>>);

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum Statement {
    Return {
        expression: Option<AstPair<Expression>>,
    },
    Assignment {
        assignee: AstPair<Assignee>,
        expression: AstPair<Expression>,
    },
    Expression {
        expression: AstPair<Expression>,
    },
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum Expression {
    Operand(Box<AstPair<Operand>>),
    Unary {
        operand: Box<AstPair<Expression>>,
        operator: Box<AstPair<UnaryOperator>>,
    },
    Binary {
        left_operand: Box<AstPair<Expression>>,
        operator: Box<AstPair<BinaryOperator>>,
        right_operand: Box<AstPair<Expression>>,
    },
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum Operand {
    Hole,
    Integer(i128),
    Float(f64),
    MatchExpression {
        condition: Box<AstPair<Expression>>,
        match_clauses: Vec<AstPair<MatchClause>>,
    },
    StructDefinition {
        fields: Vec<AstPair<Identifier>>,
    },
    EnumDefinition {
        values: Vec<AstPair<Identifier>>,
    },
    ListInit {
        items: Vec<AstPair<Expression>>,
    },
    FunctionInit {
        arguments: Vec<AstPair<Assignee>>,
        block: AstPair<Block>,
    },
    FunctionCall {
        identifier: AstPair<Identifier>,
        parameters: Vec<AstPair<Expression>>,
    },
    String(String),
    Identifier(Identifier),
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum UnaryOperator {
    Plus,
    Minus,
    Not,
    Spread,
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Exponent,
    Remainder,
    Accessor,
    Equals,
    NotEquals,
    Greater,
    Less,
    And,
    Or,
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub struct Identifier(pub String);

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub struct MatchClause {
    pub predicate_expression: Box<AstPair<Expression>>,
    pub expression: Box<AstPair<Expression>>,
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum Assignee {
    Identifier(AstPair<Identifier>),
    Pattern {
        pattern_items: Vec<AstPair<PatternItem>>,
    },
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum PatternItem {
    Hole,
    Identifier(AstPair<Identifier>),
    Spread(AstPair<Identifier>),
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub struct Span {
    pub input: String,
    pub start: usize,
    pub end: usize,
}

impl<'a> From<pest::Span<'a>> for Span {
    fn from(span: pest::Span<'a>) -> Self {
        Self {
            input: span.as_str().to_string(),
            start: span.start(),
            end: span.end(),
        }
    }
}

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub struct AstPair<A>(pub Span, pub A);

impl<T: Debug> Display for AstPair<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = format!("{:#?}", self);
        let re1 = Regex::new(r"(?mU)\n.*Span \{[\s\S]*},").unwrap();
        let no_span = re1.replace_all(s.as_str(), "");
        write!(f, "{}", no_span)
    }
}