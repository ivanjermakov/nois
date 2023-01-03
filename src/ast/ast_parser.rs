use enquote::unquote;
use pest::error::Error;
use pest::iterators::{Pair, Pairs};

use crate::ast::ast::{
    Assignee, AstPair, BinaryOperator, Block, Expression, FunctionCall, FunctionInit, Identifier,
    MatchClause, Operand, Pattern, PatternItem, PredicateExpression, Statement,
};
use crate::ast::expression::{Associativity, OperatorAssociativity, OperatorPrecedence};
use crate::ast::util::{children, custom_error, first_child, parse_children};
use crate::parser::Rule;

pub fn parse_file(pairs: &Pairs<Rule>) -> Result<AstPair<Block>, Error<Rule>> {
    parse_block(&pairs.clone().into_iter().next().unwrap())
}

pub fn parse_block(pair: &Pair<Rule>) -> Result<AstPair<Block>, Error<Rule>> {
    match pair.as_rule() {
        Rule::block => {
            let statements = parse_children(pair, parse_statement)?;
            Ok(AstPair::from_pair(pair, Block { statements }))
        }
        Rule::expression => {
            let expression = parse_expression(pair)?;
            Ok(AstPair::from_pair(
                pair,
                Block {
                    statements: vec![AstPair::from_span(
                        &expression.clone().0,
                        Statement::Expression(expression),
                    )],
                },
            ))
        }
        _ => Err(custom_error(
            pair,
            format!("expected program, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_statement(pair: &Pair<Rule>) -> Result<AstPair<Statement>, Error<Rule>> {
    match pair.as_rule() {
        Rule::return_statement => {
            let m_exp = first_child(pair).map(|p| parse_expression(&p));
            let st = if let Some(p_exp) = m_exp {
                Statement::Return(Some(p_exp?))
            } else {
                Statement::Return(None)
            };
            Ok(AstPair::from_pair(pair, st))
        }
        Rule::assignment => {
            let ch = children(pair);
            Ok(AstPair::from_pair(
                pair,
                Statement::Assignment {
                    assignee: parse_assignee(&ch[0])?,
                    expression: parse_expression(&ch[1])?,
                },
            ))
        }
        Rule::expression => {
            let exp = parse_expression(pair)?;
            Ok(AstPair::from_pair(pair, Statement::Expression(exp)))
        }
        _ => Err(custom_error(
            pair,
            format!("expected statement, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_expression(pair: &Pair<Rule>) -> Result<AstPair<Expression>, Error<Rule>> {
    match pair.as_rule() {
        Rule::expression | Rule::predicate_expression => {
            let ch = children(pair);
            if ch.len() == 1 {
                Ok(parse_expression(ch.first().unwrap())?)
            } else {
                parse_complex_expression(pair)
            }
        }
        Rule::unary_expression => {
            let ch = children(pair);
            let operator = parse_operator(&ch[0])?;
            let operand = parse_expression(&ch[1])?;
            return Ok(AstPair::from_pair(
                pair,
                Expression::Unary {
                    operator: Box::new(operator),
                    operand: Box::new(operand),
                },
            ));
        }
        Rule::match_expression => {
            let ch = children(pair);
            let condition = parse_expression(&ch[0])?;
            let match_clauses = ch
                .iter()
                .skip(1)
                .map(|c| parse_match_clause(c))
                .collect::<Result<_, _>>()?;
            return Ok(AstPair::from_pair(
                pair,
                Expression::MatchExpression {
                    condition: Box::new(condition),
                    match_clauses,
                },
            ));
        }
        _ => {
            let operand = parse_operand(pair)?;
            Ok(AstPair::from_pair(
                pair,
                Expression::Operand(Box::from(operand)),
            ))
        }
    }
}

pub fn parse_complex_expression(pair: &Pair<Rule>) -> Result<AstPair<Expression>, Error<Rule>> {
    #[derive(Debug, PartialOrd, PartialEq, Clone)]
    enum Node {
        ValueNode(ValueNode),
        ExpNode(ExpNode),
    }
    #[derive(Debug, PartialOrd, PartialEq, Clone)]
    struct ValueNode(AstPair<Expression>);
    #[derive(Debug, PartialOrd, PartialEq, Clone)]
    struct ExpNode(AstPair<BinaryOperator>, Box<Node>, Box<Node>);
    let mut operator_stack: Vec<AstPair<BinaryOperator>> = vec![];
    let mut operand_stack: Vec<Node> = vec![];
    let ch = children(pair);
    for c in ch {
        match c.as_rule() {
            Rule::binary_operator => {
                let o1: AstPair<BinaryOperator> = parse_operator(&c)?;
                let mut o2;
                while !operator_stack.is_empty() {
                    o2 = operator_stack.iter().cloned().last().unwrap();
                    if o1.1.precedence() == o2.1.precedence()
                        && o1.1.associativity() == Associativity::None
                        && o2.1.associativity() == Associativity::None
                    {
                        return Err(custom_error(
                            pair,
                            format!("Operators {} and {} cannot be chained", o1.1, o2.1),
                        ));
                    }
                    if (o1.1.associativity() != Associativity::Right
                        && o1.1.precedence() == o2.1.precedence())
                        || o1.1.precedence() < o2.1.precedence()
                    {
                        operator_stack.pop();
                        let l_op = operand_stack.pop().unwrap();
                        let r_op = operand_stack.pop().unwrap();
                        operand_stack.push(Node::ExpNode(ExpNode(
                            o2,
                            Box::from(l_op.clone()),
                            Box::from(r_op.clone()),
                        )));
                    } else {
                        break;
                    }
                }
                operator_stack.push(o1.clone());
            }
            _ => {
                let operand = parse_expression(&c)?;
                operand_stack.push(Node::ValueNode(ValueNode(operand)));
            }
        }
    }
    while !operator_stack.is_empty() {
        let l_op = operand_stack.pop().unwrap();
        let r_op = operand_stack.pop().unwrap();
        operand_stack.push(Node::ExpNode(ExpNode(
            operator_stack.pop().unwrap(),
            Box::from(l_op.clone()),
            Box::from(r_op.clone()),
        )));
    }
    fn map_node(n: &Node) -> AstPair<Expression> {
        match n {
            Node::ValueNode(ValueNode(v)) => v.clone(),
            Node::ExpNode(ExpNode(op, l, r)) => {
                let exp = Expression::Binary {
                    left_operand: Box::from(map_node(r)),
                    operator: Box::new(op.clone()),
                    right_operand: Box::from(map_node(l)),
                };
                AstPair::from_span(&op.0, exp)
            }
        }
    }
    let exp = map_node(&operand_stack.pop().unwrap());
    Ok(exp)
}

pub fn parse_operator<'a, T>(pair: &'a Pair<'_, Rule>) -> Result<AstPair<T>, Error<Rule>>
where
    T: TryFrom<Pair<'a, Rule>, Error = Error<Rule>>,
{
    let c = first_child(pair).unwrap();
    match pair.as_rule() {
        Rule::unary_operator | Rule::binary_operator => {
            c.try_into().map(|op| AstPair::from_pair(pair, op))
        }
        _ => Err(custom_error(
            pair,
            format!("expected operator, found {:?}", pair.as_rule()),
        )),
    }
}

pub fn parse_operand(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    match pair.as_rule() {
        Rule::integer => parse_integer(pair),
        Rule::float => parse_float(pair),
        Rule::string => parse_string(pair),
        Rule::HOLE_OP => Ok(AstPair::from_pair(pair, Operand::Hole)),
        Rule::function_call => parse_function_call(pair),
        Rule::function_init => parse_function_init(pair),
        Rule::list_init => parse_list_init(pair),
        Rule::struct_define => parse_struct_define(pair),
        Rule::enum_define => parse_enum_define(pair),
        Rule::identifier => {
            let id = parse_identifier(pair)?;
            Ok(AstPair::from_span(
                &id.0,
                Operand::Identifier(AstPair::from_span(&id.0, id.1)),
            ))
        }
        _ => Err(custom_error(
            pair,
            format!("expected {:?}, found {:?}", Rule::operand, pair.as_rule()),
        )),
    }
}

pub fn parse_integer(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let num_s = pair.as_str();
    let num = match num_s.parse::<i128>() {
        Ok(n) => Ok(n),
        Err(_) => Err(custom_error(
            pair,
            format!("unable to parse integer {num_s}"),
        )),
    }?;
    Ok(AstPair::from_pair(pair, Operand::Integer(num)))
}

pub fn parse_float(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let num_s = pair.as_str();
    let num = match num_s.parse::<f64>() {
        Ok(n) => Ok(n),
        Err(_) => Err(custom_error(pair, format!("unable to parse float {num_s}"))),
    }?;
    Ok(AstPair::from_pair(pair, Operand::Float(num)))
}

pub fn parse_string(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let raw_str = pair.as_str();
    let str = match unquote(raw_str) {
        Ok(s) => Ok(s),
        Err(_) => Err(custom_error(
            pair,
            format!("unable to parse string {raw_str}"),
        )),
    }?;
    Ok(AstPair::from_pair(pair, Operand::String(str)))
}

pub fn parse_function_call(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let ch = children(pair);
    Ok(AstPair::from_pair(
        pair,
        Operand::FunctionCall(FunctionCall {
            identifier: parse_identifier(&ch[0])?,
            parameters: parse_parameter_list(&ch[1])?,
        }),
    ))
}

pub fn parse_function_init(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let ch = children(pair);
    let arguments: Vec<AstPair<Assignee>> = parse_children(&ch[0], parse_assignee)?;
    let block = parse_block(&ch[1])?;
    Ok(AstPair::from_pair(
        pair,
        Operand::FunctionInit(FunctionInit { arguments, block }),
    ))
}

pub fn parse_list_init(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let items = parse_children(pair, parse_expression)?;
    Ok(AstPair::from_pair(pair, Operand::ListInit { items }))
}

pub fn parse_struct_define(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let fields = parse_children(pair, parse_identifier)?;
    Ok(AstPair::from_pair(
        pair,
        Operand::StructDefinition { fields },
    ))
}

pub fn parse_enum_define(pair: &Pair<Rule>) -> Result<AstPair<Operand>, Error<Rule>> {
    let values = parse_children(pair, parse_identifier)?;
    Ok(AstPair::from_pair(pair, Operand::EnumDefinition { values }))
}

fn parse_identifier(pair: &Pair<Rule>) -> Result<AstPair<Identifier>, Error<Rule>> {
    match pair.as_rule() {
        Rule::identifier => Ok(AstPair::from_pair(pair, Identifier::new(pair.as_str()))),
        _ => Err(custom_error(
            pair,
            format!(
                "expected {:?}, found {:?}",
                Rule::identifier,
                pair.as_rule()
            ),
        )),
    }
}

pub fn parse_parameter_list(pair: &Pair<Rule>) -> Result<Vec<AstPair<Expression>>, Error<Rule>> {
    match pair.as_rule() {
        Rule::parameter_list => parse_children(pair, parse_expression),
        _ => Err(custom_error(
            pair,
            format!(
                "expected {:?}, found {:?}",
                Rule::parameter_list,
                pair.as_rule()
            ),
        )),
    }
}

fn parse_assignee(pair: &Pair<Rule>) -> Result<AstPair<Assignee>, Error<Rule>> {
    match pair.as_rule() {
        Rule::assignee => {
            let ch = &children(pair)[0];
            let assignee = match ch.as_rule() {
                Rule::identifier => {
                    let id = parse_identifier(ch)?;
                    Assignee::Identifier(id)
                }
                Rule::HOLE_OP => Assignee::Hole,
                Rule::pattern => {
                    let pattern = parse_pattern(ch)?;
                    Assignee::Pattern(pattern)
                }
                _ => unreachable!(),
            };
            Ok(AstPair::from_pair(ch, assignee))
        }
        _ => Err(custom_error(
            pair,
            format!("expected {:?}, found {:?}", Rule::assignee, pair.as_rule()),
        )),
    }
}

fn parse_pattern(pair: &Pair<Rule>) -> Result<AstPair<Pattern>, Error<Rule>> {
    match pair.as_rule() {
        Rule::pattern => {
            let ch = children(pair);
            let pattern = if ch.len() == 1 && matches!(&ch[0].as_rule(), Rule::HOLE_OP) {
                Pattern::Hole
            } else {
                let items = ch
                    .iter()
                    .map(|c| parse_pattern_item(c))
                    .collect::<Result<_, _>>()?;
                Pattern::List(items)
            };
            Ok(AstPair::from_pair(pair, pattern))
        }
        _ => Err(custom_error(
            pair,
            format!("expected {:?}, found {:?}", Rule::pattern, pair.as_rule()),
        )),
    }
}

fn parse_pattern_item(pair: &Pair<Rule>) -> Result<AstPair<PatternItem>, Error<Rule>> {
    match pair.as_rule() {
        Rule::pattern_item => {
            let ch = &children(pair);
            let item = match ch[0].as_rule() {
                Rule::HOLE_OP => PatternItem::Hole,
                _ => {
                    let identifier = parse_identifier(&ch.last().unwrap())?;
                    PatternItem::Identifier {
                        identifier,
                        spread: ch.len() == 2 && matches!(ch[0].as_rule(), Rule::SPREAD_OP),
                    }
                }
            };
            Ok(AstPair::from_pair(pair, item))
        }
        _ => Err(custom_error(
            pair,
            format!(
                "expected {:?}, found {:?}",
                Rule::pattern_item,
                pair.as_rule()
            ),
        )),
    }
}

fn parse_match_clause(pair: &Pair<Rule>) -> Result<AstPair<MatchClause>, Error<Rule>> {
    match pair.as_rule() {
        Rule::match_clause => {
            let ch = children(pair);
            let exp = parse_predicate_expression(&ch[0])?;
            let block = parse_block(&ch[1])?;
            Ok(AstPair::from_pair(
                pair,
                MatchClause {
                    predicate_expression: Box::new(exp),
                    block: Box::new(block),
                },
            ))
        }
        _ => Err(custom_error(
            pair,
            format!(
                "expected {:?}, found {:?}",
                Rule::match_clause,
                pair.as_rule()
            ),
        )),
    }
}

fn parse_predicate_expression(
    pair: &Pair<Rule>,
) -> Result<AstPair<PredicateExpression>, Error<Rule>> {
    match pair.as_rule() {
        Rule::predicate_expression => {
            let ch = &children(pair)[0];
            let exp = match ch.as_rule() {
                Rule::pattern => PredicateExpression::Pattern(parse_pattern(ch)?),
                Rule::expression => PredicateExpression::Expression(parse_expression(ch)?),
                _ => unreachable!(),
            };
            Ok(AstPair::from_pair(ch, exp))
        }
        _ => Err(custom_error(
            pair,
            format!(
                "expected {:?}, found {:?}",
                Rule::predicate_expression,
                pair.as_rule()
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use pest::Parser;

    use crate::parser::NoisParser;

    use super::*;

    #[macro_export]
    macro_rules! match_enum {
        ($value:expr, $pattern:pat => $extracted_value:expr) => {
            match $value {
                $pattern => $extracted_value,
                _ => panic!("pattern doesn't match"),
            }
        };
    }

    #[test]
    fn build_ast_empty_block() {
        let source = "";
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        assert!(block.statements.is_empty());
    }

    #[test]
    fn build_ast_number() {
        let source = r#"
1
12.5
1e21
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        let numbers = block
            .statements
            .into_iter()
            .map(|s| {
                let exp = match_enum!(s.1, Statement::Expression(e) => e);
                let op = *match_enum!(exp.1, Expression::Operand(o) => o);
                op.1
            })
            .collect::<Vec<_>>();
        assert_eq!(numbers.len(), 3);
        assert_eq!(match_enum!(numbers[0], Operand::Integer(n) => n), 1);
        assert_eq!(match_enum!(numbers[1], Operand::Float(n) => n), 12.5);
        assert_eq!(match_enum!(numbers[2], Operand::Float(n) => n), 1e21);
    }

    #[test]
    fn build_ast_string() {
        let source = r#"
""
''
"a"
"a\nb"
'a'
'a\\\n\r\tb'
'a\u1234bc'
'hey 😎'
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        let strings: Vec<String> = block
            .statements
            .into_iter()
            .map(|s| {
                let exp = match_enum!(s.1, Statement::Expression(e) => e);
                let op = *match_enum!(exp.1, Expression::Operand(o) => o);
                match_enum!(op.1, Operand::String(s) => s)
            })
            .collect::<Vec<_>>();
        assert_eq!(strings.len(), 8);
        assert_eq!(strings[0], "");
        assert_eq!(strings[1], "");
        assert_eq!(strings[2], "a");
        assert_eq!(strings[3], "a\nb");
        assert_eq!(strings[4], "a");
        assert_eq!(strings[5], "a\\\n\r\tb");
        assert_eq!(strings[6], "a\u{1234}bc");
        assert_eq!(strings[7], "hey 😎");
    }

    #[test]
    fn build_ast_list_init() {
        let source = r#"
[]
[ ]
[,]
[1,]
[1, 2, 3]
[1, 2, 'abc']
[1, 2, 'abc',]
[
    1,
    2,
    'abc',
]
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        let get_list_items = |p: &Operand| -> Vec<Operand> {
            let exps: Vec<AstPair<Expression>> =
                match_enum!(p.clone(), Operand::ListInit{items: l} => l);
            exps.into_iter()
                .map(|e| match_enum!(e.1, Expression::Operand(o) => o).1)
                .collect()
        };
        let lists = block
            .statements
            .into_iter()
            .map(|s| {
                let exp = match_enum!(s.1, Statement::Expression(e) => e);
                let op = *match_enum!(exp.1, Expression::Operand(o) => o);
                op.1
            })
            .collect::<Vec<_>>();
        assert_eq!(lists.len(), 8);
        assert_eq!(get_list_items(&lists[0]).len(), 0);
        assert_eq!(get_list_items(&lists[1]).len(), 0);
        assert_eq!(get_list_items(&lists[2]).len(), 0);
        assert_eq!(get_list_items(&lists[3]).len(), 1);
        assert_eq!(get_list_items(&lists[4]).len(), 3);
        assert_eq!(get_list_items(&lists[5]).len(), 3);
        assert_eq!(get_list_items(&lists[6]).len(), 3);
        assert_eq!(get_list_items(&lists[7]).len(), 3);
        let l7 = get_list_items(&lists[7]);
        assert_eq!(match_enum!(l7[0], Operand::Integer(i) => i), 1);
        assert_eq!(match_enum!(l7[1], Operand::Integer(i) => i), 2);
        assert_eq!(match_enum!(&l7[2], Operand::String(s) => s), "abc");
    }

    #[test]
    fn build_ast_struct_init() {
        let source = r#"
#{a, b, c}
#{
    a,
    b,
    c
}
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        let get_struct_items = |p: &Operand| -> Vec<Identifier> {
            match_enum!(p.clone(), Operand::StructDefinition{fields: fs} => fs)
                .into_iter()
                .map(|a| a.1)
                .collect()
        };
        let structs = block
            .statements
            .into_iter()
            .map(|s| {
                let exp = match_enum!(s.1, Statement::Expression(e) => e);
                let op = *match_enum!(exp.1, Expression::Operand(o) => o);
                op.1
            })
            .collect::<Vec<_>>();
        assert_eq!(structs.len(), 2);
        assert_eq!(get_struct_items(&structs[0]).len(), 3);
        assert_eq!(get_struct_items(&structs[1]).len(), 3);
    }

    #[test]
    fn build_ast_enum_init() {
        let source = r#"
|{A, B, C}
|{
    A,
    B,
    C
}
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block = parse_file(file).unwrap().1;
        let get_enum_items = |p: &Operand| -> Vec<Identifier> {
            match_enum!(p.clone(), Operand::EnumDefinition{values: vs} => vs)
                .into_iter()
                .map(|a| a.1)
                .collect()
        };
        let enums = block
            .statements
            .into_iter()
            .map(|s| {
                let exp = match_enum!(s.1, Statement::Expression(e) => e);
                let op = *match_enum!(exp.1, Expression::Operand(o) => o);
                op.1
            })
            .collect::<Vec<_>>();
        assert_eq!(enums.len(), 2);
        assert_eq!(get_enum_items(&enums[0]).len(), 3);
        assert_eq!(get_enum_items(&enums[1]).len(), 3);
    }

    #[test]
    fn build_ast_complex_expression_basic() {
        let source = r#"a + b * c"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Binary {
                left_operand: Operand(
                    Identifier(
                        Identifier(
                            "a",
                        ),
                    ),
                ),
                operator: Add,
                right_operand: Binary {
                    left_operand: Operand(
                        Identifier(
                            Identifier(
                                "b",
                            ),
                        ),
                    ),
                    operator: Multiply,
                    right_operand: Operand(
                        Identifier(
                            Identifier(
                                "c",
                            ),
                        ),
                    ),
                },
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_complex_expression_left_associative() {
        let source = r#"a + b - c"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Binary {
                left_operand: Binary {
                    left_operand: Operand(
                        Identifier(
                            Identifier(
                                "a",
                            ),
                        ),
                    ),
                    operator: Add,
                    right_operand: Operand(
                        Identifier(
                            Identifier(
                                "b",
                            ),
                        ),
                    ),
                },
                operator: Subtract,
                right_operand: Operand(
                    Identifier(
                        Identifier(
                            "c",
                        ),
                    ),
                ),
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_complex_expression_right_associative() {
        let source = r#"a && b && c"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Binary {
                left_operand: Operand(
                    Identifier(
                        Identifier(
                            "a",
                        ),
                    ),
                ),
                operator: And,
                right_operand: Binary {
                    left_operand: Operand(
                        Identifier(
                            Identifier(
                                "b",
                            ),
                        ),
                    ),
                    operator: And,
                    right_operand: Operand(
                        Identifier(
                            Identifier(
                                "c",
                            ),
                        ),
                    ),
                },
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_complex_expression_long() {
        let source = r#"(a + b) * "abc".len() ^ 12 ^ 3 - foo(c)"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Binary {
                left_operand: Binary {
                    left_operand: Binary {
                        left_operand: Operand(
                            Identifier(
                                Identifier(
                                    "a",
                                ),
                            ),
                        ),
                        operator: Add,
                        right_operand: Operand(
                            Identifier(
                                Identifier(
                                    "b",
                                ),
                            ),
                        ),
                    },
                    operator: Multiply,
                    right_operand: Binary {
                        left_operand: Binary {
                            left_operand: Operand(
                                String(
                                    "abc",
                                ),
                            ),
                            operator: Accessor,
                            right_operand: Operand(
                                FunctionCall(
                                    FunctionCall {
                                        identifier: Identifier(
                                            "len",
                                        ),
                                        parameters: [],
                                    },
                                ),
                            ),
                        },
                        operator: Exponent,
                        right_operand: Binary {
                            left_operand: Operand(
                                Integer(
                                    12,
                                ),
                            ),
                            operator: Exponent,
                            right_operand: Operand(
                                Integer(
                                    3,
                                ),
                            ),
                        },
                    },
                },
                operator: Subtract,
                right_operand: Operand(
                    FunctionCall(
                        FunctionCall {
                            identifier: Identifier(
                                "foo",
                            ),
                            parameters: [
                                Operand(
                                    Identifier(
                                        Identifier(
                                            "c",
                                        ),
                                    ),
                                ),
                            ],
                        },
                    ),
                ),
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_complex_expression_chain_methods() {
        let source = r#"a.foo().bar(b)"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Binary {
                left_operand: Binary {
                    left_operand: Operand(
                        Identifier(
                            Identifier(
                                "a",
                            ),
                        ),
                    ),
                    operator: Accessor,
                    right_operand: Operand(
                        FunctionCall(
                            FunctionCall {
                                identifier: Identifier(
                                    "foo",
                                ),
                                parameters: [],
                            },
                        ),
                    ),
                },
                operator: Accessor,
                right_operand: Operand(
                    FunctionCall(
                        FunctionCall {
                            identifier: Identifier(
                                "bar",
                            ),
                            parameters: [
                                Operand(
                                    Identifier(
                                        Identifier(
                                            "b",
                                        ),
                                    ),
                                ),
                            ],
                        },
                    ),
                ),
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_complex_expression_non_associative_fail() {
        let source = r#"a == b <= c"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let err = parse_file(file).unwrap_err();
        assert_eq!(
            err.variant.message(),
            "Operators <= and == cannot be chained"
        );
    }

    #[test]
    fn build_ast_unary_expression() {
        let source = r#"
-2
(+a + -b)
(!a || !b) == !(a && b)
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            Unary {
                operator: Minus,
                operand: Operand(
                    Integer(
                        2,
                    ),
                ),
            },
        ),
        Expression(
            Binary {
                left_operand: Unary {
                    operator: Plus,
                    operand: Operand(
                        Identifier(
                            Identifier(
                                "a",
                            ),
                        ),
                    ),
                },
                operator: Add,
                right_operand: Unary {
                    operator: Minus,
                    operand: Operand(
                        Identifier(
                            Identifier(
                                "b",
                            ),
                        ),
                    ),
                },
            },
        ),
        Expression(
            Binary {
                left_operand: Binary {
                    left_operand: Unary {
                        operator: Not,
                        operand: Operand(
                            Identifier(
                                Identifier(
                                    "a",
                                ),
                            ),
                        ),
                    },
                    operator: Or,
                    right_operand: Unary {
                        operator: Not,
                        operand: Operand(
                            Identifier(
                                Identifier(
                                    "b",
                                ),
                            ),
                        ),
                    },
                },
                operator: Equals,
                right_operand: Unary {
                    operator: Not,
                    operand: Binary {
                        left_operand: Operand(
                            Identifier(
                                Identifier(
                                    "a",
                                ),
                            ),
                        ),
                        operator: And,
                        right_operand: Operand(
                            Identifier(
                                Identifier(
                                    "b",
                                ),
                            ),
                        ),
                    },
                },
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_match_expression_basic() {
        let source = r#"
match a {
  1 => x,
  a != 4 => x,
  _ => panic(),
}
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            MatchExpression {
                condition: Operand(
                    Identifier(
                        Identifier(
                            "a",
                        ),
                    ),
                ),
                match_clauses: [
                    MatchClause {
                        predicate_expression: Expression(
                            Operand(
                                Integer(
                                    1,
                                ),
                            ),
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Expression(
                            Binary {
                                left_operand: Operand(
                                    Identifier(
                                        Identifier(
                                            "a",
                                        ),
                                    ),
                                ),
                                operator: NotEquals,
                                right_operand: Operand(
                                    Integer(
                                        4,
                                    ),
                                ),
                            },
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Pattern(
                            Hole,
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        FunctionCall(
                                            FunctionCall {
                                                identifier: Identifier(
                                                    "panic",
                                                ),
                                                parameters: [],
                                            },
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                ],
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_match_expression_list() {
        let source = r#"
match a {
  [] => x,
  [a] => x,
  [a, _, ..t] => x,
  a.len() == 10 => x,
  _ => panic(),
}
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Expression(
            MatchExpression {
                condition: Operand(
                    Identifier(
                        Identifier(
                            "a",
                        ),
                    ),
                ),
                match_clauses: [
                    MatchClause {
                        predicate_expression: Pattern(
                            List(
                                [],
                            ),
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Pattern(
                            List(
                                [
                                    Identifier {
                                        identifier: Identifier(
                                            "a",
                                        ),
                                        spread: false,
                                    },
                                ],
                            ),
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Pattern(
                            List(
                                [
                                    Identifier {
                                        identifier: Identifier(
                                            "a",
                                        ),
                                        spread: false,
                                    },
                                    Hole,
                                    Identifier {
                                        identifier: Identifier(
                                            "t",
                                        ),
                                        spread: true,
                                    },
                                ],
                            ),
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Expression(
                            Binary {
                                left_operand: Binary {
                                    left_operand: Operand(
                                        Identifier(
                                            Identifier(
                                                "a",
                                            ),
                                        ),
                                    ),
                                    operator: Accessor,
                                    right_operand: Operand(
                                        FunctionCall(
                                            FunctionCall {
                                                identifier: Identifier(
                                                    "len",
                                                ),
                                                parameters: [],
                                            },
                                        ),
                                    ),
                                },
                                operator: Equals,
                                right_operand: Operand(
                                    Integer(
                                        10,
                                    ),
                                ),
                            },
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        Identifier(
                                            Identifier(
                                                "x",
                                            ),
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                    MatchClause {
                        predicate_expression: Pattern(
                            Hole,
                        ),
                        block: Block {
                            statements: [
                                Expression(
                                    Operand(
                                        FunctionCall(
                                            FunctionCall {
                                                identifier: Identifier(
                                                    "panic",
                                                ),
                                                parameters: [],
                                            },
                                        ),
                                    ),
                                ),
                            ],
                        },
                    },
                ],
            },
        ),
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }

    #[test]
    fn build_ast_assignee() {
        let source = r#"
a = []
[] = []
[a] = []
[..as] = []
[a, b,] = []
[a, b, ..cs] = []
_ = []
"#;
        let file = &NoisParser::parse(Rule::program, source).unwrap();
        let block: AstPair<Block> = parse_file(file).unwrap();
        let expect = r#"
Block {
    statements: [
        Assignment {
            assignee: Identifier(
                Identifier(
                    "a",
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                List(
                    [],
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                List(
                    [
                        Identifier {
                            identifier: Identifier(
                                "a",
                            ),
                            spread: false,
                        },
                    ],
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                List(
                    [
                        Identifier {
                            identifier: Identifier(
                                "as",
                            ),
                            spread: true,
                        },
                    ],
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                List(
                    [
                        Identifier {
                            identifier: Identifier(
                                "a",
                            ),
                            spread: false,
                        },
                        Identifier {
                            identifier: Identifier(
                                "b",
                            ),
                            spread: false,
                        },
                    ],
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                List(
                    [
                        Identifier {
                            identifier: Identifier(
                                "a",
                            ),
                            spread: false,
                        },
                        Identifier {
                            identifier: Identifier(
                                "b",
                            ),
                            spread: false,
                        },
                        Identifier {
                            identifier: Identifier(
                                "cs",
                            ),
                            spread: true,
                        },
                    ],
                ),
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
        Assignment {
            assignee: Pattern(
                Hole,
            ),
            expression: Operand(
                ListInit {
                    items: [],
                },
            ),
        },
    ],
}
"#;
        assert_eq!(format!("{:#?}", block), expect.trim())
    }
}
