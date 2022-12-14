use std::cell::RefMut;
use std::collections::HashMap;

use log::debug;

use crate::ast::ast::{AstPair, Identifier};
use crate::error::Error;
use crate::interpret::context::{Context, Definition};
use crate::interpret::evaluate::Evaluate;
use crate::interpret::value::Value;
use crate::stdlib::*;
use crate::util::vec_to_string_paren;

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub definitions: HashMap<Identifier, Definition>,
}

pub fn stdlib() -> Vec<Package> {
    vec![
        io::package(),
        binary_operator::package(),
        unary_operator::package(),
        list::package(),
        value::package(),
        option::package(),
    ]
}

pub trait LibFunction {
    fn name() -> String;

    // TODO: use patterns to validate call args
    fn call(args: &Vec<AstPair<Value>>, ctx: &mut RefMut<Context>) -> Result<Value, Error>;

    fn call_fn(
        args: Vec<AstPair<Value>>,
        ctx: &mut RefMut<Context>,
    ) -> Result<AstPair<Value>, Error> {
        let arguments: Vec<AstPair<Value>> = args
            .iter()
            .map(|a| a.eval(ctx, false))
            .collect::<Result<_, _>>()?;

        let res = Self::call(&arguments, ctx);
        debug!(
            "stdlib function call {:?}, args: {:?}, result: {:?}",
            Self::name(),
            &arguments,
            &res
        );

        let scope = ctx.scope_stack.last().unwrap();
        let callee = scope
            .method_callee
            .clone()
            .map(|c| c.0)
            .or(scope.callee.clone())
            .expect("callee not found");
        res.map(|v| AstPair::from_span(&callee, v))
    }

    fn definition() -> (Identifier, Definition) {
        (
            Identifier(Self::name()),
            Definition::System(|args, ctx| Self::call_fn(args, ctx)),
        )
    }
}

pub fn arg_error(
    expected_type: &str,
    args: &Vec<AstPair<Value>>,
    ctx: &mut RefMut<Context>,
) -> Error {
    Error::from_callee(
        ctx,
        format!(
            "expected {}, found {}",
            expected_type,
            vec_to_string_paren(args.into_iter().map(|l| l.1.value_type()).collect())
        ),
    )
}
