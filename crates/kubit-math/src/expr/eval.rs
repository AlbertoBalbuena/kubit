//! Evaluation with dimensional analysis.

use super::ast::{BinOp, Expr, ExprKind, Func};
use super::error::{ExprError, ExprErrorKind};
use super::parser::parse;
use super::quantity::{Dim, Quantity};
use crate::units::Unit;
use core::f64::consts::PI;
use std::collections::BTreeMap;

/// Resolves identifiers (parameters, measures like `Body1.height`) while evaluating.
pub trait Context {
    /// The value of `name`, or `None` if unknown.
    fn resolve(&self, name: &str) -> Option<Quantity>;
}

/// A context with no identifiers.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmptyContext;

impl Context for EmptyContext {
    fn resolve(&self, _name: &str) -> Option<Quantity> {
        None
    }
}

impl Context for BTreeMap<String, Quantity> {
    fn resolve(&self, name: &str) -> Option<Quantity> {
        self.get(name).copied()
    }
}

/// Parses and evaluates `src` in one step.
pub fn evaluate(src: &str, ctx: &dyn Context) -> Result<Quantity, ExprError> {
    parse(src)?.eval(ctx)
}

impl Expr {
    /// Evaluates the expression. Every intermediate value is checked to be finite and bounded.
    pub fn eval(&self, ctx: &dyn Context) -> Result<Quantity, ExprError> {
        eval_node(self, ctx)
    }
}

fn eval_node(e: &Expr, ctx: &dyn Context) -> Result<Quantity, ExprError> {
    let wrap = |k: ExprErrorKind| ExprError::new(k, e.span);
    match &e.kind {
        ExprKind::Number(v) => Quantity::scalar(*v).checked().map_err(wrap),
        ExprKind::Pi => Ok(Quantity::scalar(PI)),
        ExprKind::Ident(name) => ctx
            .resolve(name)
            .ok_or_else(|| wrap(ExprErrorKind::UnknownIdentifier { name: name.clone() }))?
            .checked()
            .map_err(wrap),
        ExprKind::Neg(inner) => {
            let q = eval_node(inner, ctx)?;
            Ok(Quantity::new(-q.value, q.dim))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            let a = eval_node(lhs, ctx)?;
            let b = eval_node(rhs, ctx)?;
            binary(*op, a, b).and_then(Quantity::checked).map_err(wrap)
        }
        ExprKind::Unit { expr, unit } => {
            let q = eval_node(expr, ctx)?;
            if q.dim != Dim::SCALAR {
                return Err(wrap(ExprErrorKind::InvalidOperandDimension {
                    context: "a unit suffix needs a plain number",
                    found: q.dim,
                }));
            }
            let (factor, dim) = match unit {
                Unit::Length(u) => (u.mm_per_unit(), Dim::LENGTH),
                Unit::Angle(u) => (u.rad_per_unit(), Dim::ANGLE),
            };
            Quantity::new(q.value * factor, dim).checked().map_err(wrap)
        }
        ExprKind::Call { func, args } => {
            let vals = args
                .iter()
                .map(|a| eval_node(a, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            call(*func, &vals).and_then(Quantity::checked).map_err(wrap)
        }
    }
}

fn mismatch(a: Quantity, b: Quantity) -> ExprErrorKind {
    ExprErrorKind::DimensionMismatch {
        left: a.dim,
        right: b.dim,
    }
}

fn binary(op: BinOp, a: Quantity, b: Quantity) -> Result<Quantity, ExprErrorKind> {
    match op {
        BinOp::Add | BinOp::Sub => {
            if a.dim != b.dim {
                return Err(mismatch(a, b));
            }
            let v = if op == BinOp::Add {
                a.value + b.value
            } else {
                a.value - b.value
            };
            Ok(Quantity::new(v, a.dim))
        }
        BinOp::Mul => {
            let dim = a
                .dim
                .checked_add(b.dim)
                .ok_or(ExprErrorKind::ExponentOverflow)?;
            Ok(Quantity::new(a.value * b.value, dim))
        }
        BinOp::Div => {
            if b.value == 0.0 {
                return Err(ExprErrorKind::DivisionByZero);
            }
            let dim = a
                .dim
                .checked_sub(b.dim)
                .ok_or(ExprErrorKind::ExponentOverflow)?;
            Ok(Quantity::new(a.value / b.value, dim))
        }
        BinOp::Pow => pow(a, b),
    }
}

fn pow(base: Quantity, exp: Quantity) -> Result<Quantity, ExprErrorKind> {
    if exp.dim != Dim::SCALAR {
        return Err(ExprErrorKind::InvalidOperandDimension {
            context: "an exponent must be a plain number",
            found: exp.dim,
        });
    }
    if base.dim == Dim::SCALAR {
        if base.value == 0.0 && exp.value < 0.0 {
            return Err(ExprErrorKind::DivisionByZero);
        }
        let v = base.value.powf(exp.value);
        return if v.is_nan() {
            Err(ExprErrorKind::DomainError { function: "^" })
        } else {
            Ok(Quantity::scalar(v)) // infinities are rejected by the caller's range check
        };
    }
    let n = exp.value;
    let limit = f64::from(super::MAX_DIM_EXPONENT);
    if n.fract() != 0.0 || n.abs() > limit {
        return Err(ExprErrorKind::InvalidOperandDimension {
            context: "the exponent of a dimensioned value must be an integer in -6..=6",
            found: base.dim,
        });
    }
    let n = n as i8;
    let dim = base
        .dim
        .checked_scale(n)
        .ok_or(ExprErrorKind::ExponentOverflow)?;
    if base.value == 0.0 && n < 0 {
        return Err(ExprErrorKind::DivisionByZero);
    }
    Ok(Quantity::new(base.value.powi(i32::from(n)), dim))
}

fn call(func: Func, args: &[Quantity]) -> Result<Quantity, ExprErrorKind> {
    let Some(&first) = args.first() else {
        return Err(ExprErrorKind::UnexpectedEnd {
            expected: "a function argument",
        });
    };
    match func {
        Func::Sin | Func::Cos | Func::Tan => {
            if first.dim != Dim::ANGLE && first.dim != Dim::SCALAR {
                return Err(ExprErrorKind::InvalidOperandDimension {
                    context: "trigonometric functions need an angle (or a plain number in radians)",
                    found: first.dim,
                });
            }
            let v = match func {
                Func::Sin => first.value.sin(),
                Func::Cos => first.value.cos(),
                _ => first.value.tan(),
            };
            Ok(Quantity::scalar(v))
        }
        Func::Sqrt => {
            if first.value < 0.0 {
                return Err(ExprErrorKind::DomainError { function: "sqrt" });
            }
            let dim = first
                .dim
                .halved()
                .ok_or(ExprErrorKind::InvalidOperandDimension {
                    context: "sqrt needs even dimension exponents",
                    found: first.dim,
                })?;
            Ok(Quantity::new(first.value.sqrt(), dim))
        }
        Func::Abs => Ok(Quantity::new(first.value.abs(), first.dim)),
        Func::Round => Ok(Quantity::new(first.value.round(), first.dim)),
        Func::Floor => Ok(Quantity::new(first.value.floor(), first.dim)),
        Func::Ceil => Ok(Quantity::new(first.value.ceil(), first.dim)),
        Func::Min | Func::Max => {
            let mut best = first;
            for &q in args.iter().skip(1) {
                if q.dim != best.dim {
                    return Err(mismatch(best, q));
                }
                let better = if func == Func::Min {
                    q.value < best.value
                } else {
                    q.value > best.value
                };
                if better {
                    best = q;
                }
            }
            Ok(best)
        }
    }
}
