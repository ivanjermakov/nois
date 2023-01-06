use std::cell::RefMut;

use log::debug;
use pest::error::Error;

use crate::ast::ast::{
    Assignee, AstPair, Expression, Identifier, MatchClause, Pattern, PredicateExpression,
};
use crate::interpret::context::{Context, Definition};
use crate::interpret::evaluate::Evaluate;
use crate::interpret::value::Value;
use crate::parser::Rule;

pub fn match_expression(
    expression: AstPair<Expression>,
    ctx: &mut RefMut<Context>,
) -> Result<Option<(AstPair<MatchClause>, Vec<(Identifier, Definition)>)>, Error<Rule>> {
    match expression.1 {
        Expression::MatchExpression {
            condition,
            match_clauses,
        } => {
            let value = condition.eval(ctx, true)?;
            for (i, clause) in match_clauses.into_iter().enumerate() {
                let p_match = match_predicate(value.clone(), clause.clone(), ctx)?;
                if let Some(pm) = p_match {
                    debug!("matched pattern #{i}: {:?}", clause.1);
                    return Ok(Some((clause, pm)));
                }
            }
            return Ok(None);
        }
        _ => unreachable!(),
    }
}

pub fn match_predicate(
    value: AstPair<Value>,
    clause: AstPair<MatchClause>,
    ctx: &mut RefMut<Context>,
) -> Result<Option<Vec<(Identifier, Definition)>>, Error<Rule>> {
    debug!("matching {:?} against {:?}", &value, &clause);
    match clause.1.predicate_expression.1 {
        PredicateExpression::Expression(exp) => {
            let target = exp.eval(ctx, true)?;
            Ok(if value.1 == target.1 {
                Some(vec![])
            } else {
                None
            })
        }
        PredicateExpression::Assignee(assignee) => match assignee.1 {
            Assignee::Hole => Ok(Some(vec![])),
            Assignee::Identifier(_) => assign_value_definitions(&assignee, value).map(|r| Some(r)),
            Assignee::Pattern(p) => match p.1 {
                Pattern::Hole => Ok(Some(vec![])),
                Pattern::List(_) => todo!("patterns"),
            },
        },
    }
}

pub fn assign_expression_definitions(
    assignee: AstPair<Assignee>,
    expression: AstPair<Expression>,
) -> Result<Vec<(Identifier, Definition)>, Error<Rule>> {
    match assignee.clone().1 {
        Assignee::Identifier(i) => Ok(vec![(i.clone().1, Definition::User(i, expression))]),
        Assignee::Hole => Ok(vec![]),
        Assignee::Pattern(_) => todo!("patterns"),
    }
}

pub fn assign_value_definitions(
    assignee: &AstPair<Assignee>,
    value: AstPair<Value>,
) -> Result<Vec<(Identifier, Definition)>, Error<Rule>> {
    match assignee.clone().1 {
        Assignee::Identifier(i) => Ok(vec![(i.clone().1, Definition::Value(value))]),
        Assignee::Hole => Ok(vec![]),
        Assignee::Pattern(_) => todo!("patterns"),
    }
}