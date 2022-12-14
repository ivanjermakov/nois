use std::cell::RefMut;
use std::iter::zip;

use log::debug;

use crate::ast::ast::{AstPair, Expression, Identifier, MatchClause, PatternItem};
use crate::error::Error;
use crate::interpret::context::{Context, Definition};
use crate::interpret::evaluate::Evaluate;
use crate::interpret::value::Value;

pub fn match_expression(
    expression: AstPair<Expression>,
    ctx: &mut RefMut<Context>,
) -> Result<Option<(AstPair<MatchClause>, Vec<(Identifier, Definition)>)>, Error> {
    match expression.1 {
        Expression::MatchExpression {
            condition,
            match_clauses,
        } => {
            let value = condition.eval(ctx, true)?;
            for (i, clause) in match_clauses.into_iter().enumerate() {
                debug!("matching {:?} against {:?}", &value, &clause);
                let p_match = match_pattern_item(value.clone(), clause.1.pattern.clone(), ctx)?;
                if let Some(pm) = p_match {
                    debug!("matched pattern #{i}: {:?}", clause.1);
                    return Ok(Some((clause, pm)));
                }
            }
            Ok(None)
        }
        _ => unreachable!(),
    }
}

pub fn match_pattern_item(
    value: AstPair<Value>,
    pattern_item: AstPair<PatternItem>,
    ctx: &mut RefMut<Context>,
) -> Result<Option<Vec<(Identifier, Definition)>>, Error> {
    let defs = match pattern_item.1 {
        PatternItem::Hole => Some(vec![]),
        PatternItem::Integer(_)
        | PatternItem::Float(_)
        | PatternItem::Boolean(_)
        | PatternItem::String(_) => Value::try_from(pattern_item.clone())
            .map_err(|e| Error::from_span(&pattern_item.0, &ctx.ast_context, e))?
            .eq(&value.1)
            .then(|| vec![]),
        PatternItem::Identifier {
            identifier: id,
            spread: false,
        } => Some(vec![(id.1, Definition::Value(value))]),
        PatternItem::Identifier {
            identifier: _,
            spread: true,
        } => {
            return Err(Error::from_span(
                &pattern_item.0,
                &ctx.ast_context,
                format!("unexpected spread operator"),
            ));
        }
        PatternItem::PatternList(items) => {
            return match &value.1 {
                Value::List { items: vs, .. } => {
                    let spread_items = items
                        .iter()
                        .enumerate()
                        .filter_map(|(i, id)| match &id.1 {
                            PatternItem::Identifier {
                                identifier,
                                spread: true,
                            } => Some((i, identifier)),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    match spread_items.len() {
                        0 => match_list(&value, ctx, items, vs),
                        1 => match_list_with_spread(
                            &value,
                            ctx,
                            items.clone(),
                            vs,
                            *spread_items.first().unwrap(),
                        ),
                        _ => Err(Error::from_span(
                            &pattern_item.0,
                            &ctx.ast_context,
                            format!("ambiguous spreading logic: single spread identifier allowed"),
                        )),
                    }
                }
                _ => Err(Error::from_span(
                    &value.0,
                    &ctx.ast_context,
                    format!("expected [*] to deconstruct, got {:?}", value.1),
                )),
            };
        }
    };
    Ok(defs)
}

fn match_list(
    value: &AstPair<Value>,
    ctx: &mut RefMut<Context>,
    items: Vec<AstPair<PatternItem>>,
    vs: &Vec<Value>,
) -> Result<Option<Vec<(Identifier, Definition)>>, Error> {
    if items.len() == vs.len() {
        Ok(zip(items, vs)
            .map(|(i, v)| match_pattern_item(value.map(|_| v.clone()), i, ctx))
            .collect::<Result<Option<Vec<_>>, _>>()?
            .map(|o| o.into_iter().flatten().collect::<Vec<_>>()))
    } else {
        Err(Error::from_span(
            &value.0,
            &ctx.ast_context,
            format!(
                "incompatible deconstruction length: expected {}, got {}",
                items.len(),
                vs.len()
            ),
        ))
    }
}

fn match_list_with_spread(
    value: &AstPair<Value>,
    ctx: &mut RefMut<Context>,
    items: Vec<AstPair<PatternItem>>,
    vs: &Vec<Value>,
    spread_item: (usize, &AstPair<Identifier>),
) -> Result<Option<Vec<(Identifier, Definition)>>, Error> {
    let before_pairs = items
        .iter()
        .take(spread_item.0)
        .cloned()
        .zip(vs.iter().take(spread_item.0))
        .map(|(i, v)| match_pattern_item(value.map(|_| v.clone()), i, ctx))
        .collect::<Result<Option<Vec<_>>, _>>()?
        .map(|o| o.into_iter().flatten().collect::<Vec<_>>());
    let spread_value_count = vs.len() - (items.len() - 1);
    let spread_values = vs
        .iter()
        .skip(spread_item.0)
        .take(spread_value_count)
        .cloned()
        .collect::<Vec<_>>();
    let spread_pair = Some(vec![(
        spread_item.1.clone().1,
        Definition::Value(value.map(|_| Value::list(spread_values.clone()))),
    )]);
    let after_pairs = items
        .iter()
        .skip(spread_item.0 + 1)
        .cloned()
        .zip(vs.iter().skip(spread_value_count + spread_item.0))
        .map(|(i, v)| match_pattern_item(value.map(|_| v.clone()), i, ctx))
        .collect::<Result<Option<Vec<_>>, _>>()?
        .map(|o| o.into_iter().flatten().collect::<Vec<_>>());
    Ok(vec![before_pairs, spread_pair, after_pairs]
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .map(|l| l.into_iter().flatten().collect()))
}
