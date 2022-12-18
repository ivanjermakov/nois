use crate::parser::Rule;
use pest::error::{Error, ErrorVariant};
use pest::iterators::Pair;
use regex::Regex;
use std::fmt;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug, PartialOrd, PartialEq, Clone)]
pub enum Program {
    Block { statements: Vec<AstPair<Statement>> },
}

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
    Number(i128),
    MatchExpression {
        condition: Box<AstPair<Expression>>,
        match_clauses: Vec<AstPair<MatchClause>>,
    },
    StructDefinition {
        identifier: AstPair<Identifier>,
        field_identifiers: Vec<AstPair<Identifier>>,
    },
    EnumDefinition {
        identifier: AstPair<Identifier>,
        value_identifiers: Vec<AstPair<Identifier>>,
    },
    ListInit {
        items: Vec<AstPair<Expression>>,
    },
    FunctionInit {
        arguments: Vec<AstPair<Assignee>>,
        statements: Vec<AstPair<Statement>>,
    },
    FunctionCall {
        identifier: AstPair<Identifier>,
        parameters: Vec<AstPair<Expression>>,
    },
    String(String),
    Identifier(String),
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
    input: String,
    start: usize,
    end: usize,
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
pub struct AstPair<A>(Span, A);

impl<T: Debug> Display for AstPair<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let s = format!("{:#?}", self);
        let re1 = Regex::new(r"(?mU)\n.*Span \{[\s\S]*},").unwrap();
        let no_span = re1.replace_all(s.as_str(), "");
        write!(f, "{}", no_span)
    }
}

pub fn new_pair<A>(p: &Pair<Rule>, ast: A) -> AstPair<A> {
    AstPair(p.as_span().into(), ast)
}

fn custom_error(pair: &Pair<Rule>, message: String) -> Error<Rule> {
    Error::new_from_span(ErrorVariant::CustomError { message }, pair.as_span())
}

pub fn parse_program(pair: &Pair<Rule>) -> Result<AstPair<Program>, Error<Rule>> {
    match pair.as_rule() {
        Rule::block => {
            let mut statements: Vec<AstPair<Statement>> = vec![];
            for statement in pair.clone().into_inner().map(|s| parse_statement(&s)) {
                match statement {
                    Ok(s) => statements.push(s),
                    Err(e) => return Err(e),
                }
            }
            Ok(new_pair(pair, Program::Block { statements }))
        }
        _ => Err(custom_error(
            pair,
            format!("expected program, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_statement(pair: &Pair<Rule>) -> Result<AstPair<Statement>, Error<Rule>> {
    match pair.as_rule() {
        Rule::return_statement => todo!(),
        Rule::assignment => todo!(),
        Rule::expression => parse_expression(pair)
            .map(|expression| new_pair(pair, Statement::Expression { expression })),
        _ => Err(custom_error(
            pair,
            format!("expected statement, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_expression(pair: &Pair<Rule>) -> Result<AstPair<Expression>, Error<Rule>> {
    match pair.as_rule() {
        Rule::expression => {
            let children = pair.clone().into_inner().collect::<Vec<_>>();
            if children.len() == 1 {
                return parse_operand(&children[0])
                    .map(|o| new_pair(pair, Expression::Operand(Box::from(o))));
            }
            todo!("other expression logic")
        }
        _ => Err(custom_error(
            pair,
            format!("expected expression, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_operand(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    match pair.as_rule() {
        Rule::number => Ok(new_pair(pair, Operand::Number(42))),
        _ => Err(custom_error(
            pair,
            format!("expected expression, found {:?}", pair.as_rule()),
        )),
    }
}