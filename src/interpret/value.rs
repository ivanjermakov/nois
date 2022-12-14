use std::collections::HashSet;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops;

use num::NumCast;

use crate::ast::ast::{AstPair, FunctionInit, PatternItem, UnaryOperator, ValueType};

#[derive(Debug, PartialOrd, Clone)]
pub enum Value {
    Unit,
    I(i128),
    F(f64),
    C(char),
    B(bool),
    List { items: Vec<Value>, spread: bool },
    // TODO: closures don't remember their scope
    Fn(FunctionInit),
    Type(ValueType),
}

impl Value {
    pub fn value_type(&self) -> Value {
        let vt = match self {
            Value::Unit => ValueType::Unit,
            Value::I(_) => ValueType::Integer,
            Value::F(_) => ValueType::Float,
            Value::C(_) => ValueType::Char,
            Value::B(_) => ValueType::Boolean,
            Value::Fn(_) => ValueType::Function,
            Value::Type(_) => ValueType::Type,
            Value::List { items, .. } => {
                if items.is_empty() {
                    return Value::List {
                        items: vec![Value::Type(ValueType::Any)],
                        spread: false,
                    };
                }
                let types: Vec<Value> = items.into_iter().map(|v| v.value_type()).collect();
                return if types.iter().collect::<HashSet<_>>().len() == 1 {
                    Value::List {
                        items: vec![types[0].clone()],
                        spread: false,
                    }
                } else {
                    Value::List {
                        items: types.clone(),
                        spread: false,
                    }
                };
            }
        };
        Value::Type(vt)
    }

    pub fn to(&self, vt: &Value) -> Option<Self> {
        let arg_type = self.value_type();
        if &arg_type == vt {
            return Some(self.clone());
        }
        match (self, vt) {
            // cast to [C]
            (arg, Value::List { items, .. }) => match &items[0] {
                Value::Type(t) => {
                    let str = match t {
                        ValueType::Char => match arg {
                            Value::I(a) => Some(format!("{a}")),
                            Value::F(a) => Some(format!("{a}")),
                            Value::C(a) => Some(format!("{a}")),
                            _ => None,
                        },
                        _ => None,
                    };
                    str.map(|s| Value::List {
                        items: s.chars().into_iter().map(|c| Value::C(c)).collect(),
                        spread: false,
                    })
                }
                _ => None,
            },
            (arg, Value::Type(t)) => match (arg, t) {
                // cast from [C]
                (Value::List { .. }, t)
                    if arg_type
                        == Value::List {
                            items: vec![Value::Type(ValueType::Char)],
                            spread: false,
                        } =>
                {
                    let s = arg.to_string();
                    match t {
                        ValueType::Unit => Some(Value::Unit),
                        ValueType::Integer => s.parse().map(|i| Value::I(i)).ok(),
                        ValueType::Float => s.parse().map(|f| Value::F(f)).ok(),
                        ValueType::Char => s.parse().map(|c| Value::C(c)).ok(),
                        ValueType::Boolean => match s.as_str() {
                            "True" => Some(Value::B(true)),
                            "False" => Some(Value::B(false)),
                            _ => None,
                        },
                        _ => None,
                    }
                }
                // mono-type casts
                _ => match (arg, t) {
                    (Value::I(i), ValueType::Float) => {
                        <f64 as NumCast>::from(*i).map(|f| Value::F(f))
                    }
                    (Value::F(f), ValueType::Integer) => {
                        <i128 as NumCast>::from(*f).map(|i| Value::I(i))
                    }
                    (Value::I(i), ValueType::Char) => <u32 as NumCast>::from(*i)
                        .and_then(|u| char::try_from(u).map(|c| Value::C(c)).ok()),
                    (Value::C(c), ValueType::Integer) => {
                        <u32>::try_from(*c).ok().map(|u| Value::I(u as i128))
                    }
                    _ => None,
                },
            },
            (_, _) => None,
        }
    }

    pub fn list(vec: Vec<Value>) -> Value {
        Self::List {
            items: vec,
            spread: false,
        }
    }
}

impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        format!("{:?}", self).hash(state);
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Type(ValueType::Any), Self::Type(_) | Self::List { .. }) => true,
            (Self::Type(_) | Self::List { .. }, Self::Type(ValueType::Any)) => true,
            (Self::Type(a), Self::Type(b)) => a == b,
            (
                Self::List {
                    items: ia,
                    spread: sa,
                },
                Self::List {
                    items: ib,
                    spread: sb,
                },
            ) => ia == ib && sa == sb,
            (Self::Fn(a), Self::Fn(b)) => a == b,
            _ => format!("{:?}", self) == format!("{:?}", other),
        }
    }
}

impl Eq for Value {}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Value::Unit => write!(f, "()"),
            Value::I(i) => write!(f, "{i}"),
            Value::F(fl) => write!(f, "{fl}"),
            Value::C(c) => write!(f, "{c}"),
            Value::B(b) => write!(f, "{}", if *b { "True" } else { "False" }),
            Value::List { items: l, spread } => {
                let all_c = !l.is_empty() && l.iter().all(|v| matches!(v, Value::C(_)));
                let is = l.into_iter().map(|i| format!("{}", i)).collect::<Vec<_>>();
                let spread_s = if *spread {
                    UnaryOperator::Spread.to_string()
                } else {
                    "".to_string()
                };
                if all_c && !*spread {
                    write!(f, "{}", is.join(""))
                } else {
                    write!(f, "{}[{}]", spread_s, is.join(", "))
                }
            }
            Value::Fn(_) => write!(f, "<fn>"),
            Value::Type(vt) => write!(f, "{vt}"),
        }
    }
}

impl TryFrom<AstPair<PatternItem>> for Value {
    type Error = String;

    fn try_from(a: AstPair<PatternItem>) -> Result<Self, Self::Error> {
        match a.1 {
            PatternItem::Integer(i) => Ok(Value::I(i)),
            PatternItem::Float(f) => Ok(Value::F(f)),
            PatternItem::Boolean(b) => Ok(Value::B(b)),
            PatternItem::String(s) => Ok(Value::List {
                items: s.chars().map(|c| Value::C(c)).collect(),
                spread: false,
            }),
            _ => Err(format!(
                "unable to convert pattern item {:?} into value",
                a.1
            )),
        }
    }
}

impl ops::Add for Value {
    type Output = Result<Value, String>;

    fn add(self, rhs: Self) -> Self::Output {
        fn push_end(a: &Vec<Value>, b: &Value) -> Value {
            Value::List {
                items: a
                    .into_iter()
                    .cloned()
                    .chain(vec![b.clone()].into_iter())
                    .collect(),
                spread: false,
            }
        }
        fn push_start(a: &Value, b: &Vec<Value>) -> Value {
            Value::List {
                items: vec![a.clone()]
                    .into_iter()
                    .chain(b.clone().into_iter())
                    .collect(),
                spread: false,
            }
        }
        fn _add(a: &Value, b: &Value) -> Option<Value> {
            match (a, b) {
                (Value::I(i1), Value::I(i2)) => Some(Value::I(i1 + i2)),
                (Value::F(f1), Value::F(f2)) => Some(Value::F(f1 + f2)),
                (Value::I(i1), Value::F(f2)) => Some(Value::F(*i1 as f64 + f2)),
                (
                    Value::List {
                        items: l1,
                        spread: s1,
                    },
                    Value::List {
                        items: l2,
                        spread: s2,
                    },
                ) => match (s1, s2) {
                    _ if s1 == s2 => Some(Value::List {
                        items: l1
                            .clone()
                            .into_iter()
                            .chain(l2.clone().into_iter())
                            .collect(),
                        spread: false,
                    }),
                    (true, false) => Some(push_end(l1, b)),
                    (false, true) => Some(push_start(a, l2)),
                    _ => unreachable!(),
                },
                (Value::List { items: l1, .. }, _) => Some(push_end(l1, b)),
                (_, Value::List { items: l2, .. }) => Some(push_start(a, l2)),
                _ => None,
            }
        }
        match _add(&self, &rhs).or(_add(&rhs, &self)) {
            Some(r) => Ok(r),
            None => Err(format!(
                "incompatible operands: {} + {}",
                self.value_type(),
                rhs.value_type()
            )),
        }
    }
}

impl ops::Sub for Value {
    type Output = Result<Value, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        fn _sub(a: &Value, b: &Value) -> Option<Value> {
            match (a, b) {
                (Value::I(i1), Value::I(i2)) => Some(Value::I(i1 - i2)),
                (Value::F(f1), Value::F(f2)) => Some(Value::F(f1 - f2)),
                (Value::I(i1), Value::F(f2)) => Some(Value::F(*i1 as f64 - f2)),
                _ => None,
            }
        }
        match _sub(&self, &rhs).or(_sub(&rhs, &self)) {
            Some(r) => Ok(r),
            None => Err(format!(
                "incompatible operands: {} - {}",
                self.value_type(),
                rhs.value_type()
            )),
        }
    }
}

impl ops::Rem for Value {
    type Output = Result<Value, String>;

    fn rem(self, rhs: Self) -> Self::Output {
        fn _rem(a: &Value, b: &Value) -> Option<Value> {
            match (a, b) {
                (Value::I(i1), Value::I(i2)) => Some(Value::I(i1 % i2)),
                (Value::F(f1), Value::F(f2)) => Some(Value::F(f1 % f2)),
                (Value::I(i1), Value::F(f2)) => Some(Value::F(*i1 as f64 % f2)),
                _ => None,
            }
        }
        match _rem(&self, &rhs).or(_rem(&rhs, &self)) {
            Some(r) => Ok(r),
            None => Err(format!(
                "incompatible operands: {} % {}",
                self.value_type(),
                rhs.value_type()
            )),
        }
    }
}
