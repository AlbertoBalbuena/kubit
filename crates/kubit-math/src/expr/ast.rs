//! Syntax tree.

use super::MAX_CALL_ARGS;
use super::error::Span;
use crate::units::Unit;
use std::collections::BTreeSet;

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `^`
    Pow,
}

/// Built-in functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    /// `sin(angle)`
    Sin,
    /// `cos(angle)`
    Cos,
    /// `tan(angle)`
    Tan,
    /// `sqrt(x)`
    Sqrt,
    /// `abs(x)`
    Abs,
    /// `min(a, b, ...)`
    Min,
    /// `max(a, b, ...)`
    Max,
    /// `round(x)`
    Round,
    /// `floor(x)`
    Floor,
    /// `ceil(x)`
    Ceil,
}

impl Func {
    /// Looks a function up by its (case-sensitive) name.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "sqrt" => Self::Sqrt,
            "abs" => Self::Abs,
            "min" => Self::Min,
            "max" => Self::Max,
            "round" => Self::Round,
            "floor" => Self::Floor,
            "ceil" => Self::Ceil,
            _ => return None,
        })
    }

    /// The function's name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Sin => "sin",
            Self::Cos => "cos",
            Self::Tan => "tan",
            Self::Sqrt => "sqrt",
            Self::Abs => "abs",
            Self::Min => "min",
            Self::Max => "max",
            Self::Round => "round",
            Self::Floor => "floor",
            Self::Ceil => "ceil",
        }
    }

    /// Inclusive argument count range.
    pub const fn arity(self) -> (usize, usize) {
        match self {
            Self::Min | Self::Max => (1, MAX_CALL_ARGS),
            _ => (1, 1),
        }
    }
}

/// A node of the syntax tree, with the span of source it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// What the node is.
    pub kind: ExprKind,
    /// Source span.
    pub span: Span,
}

/// Node variants.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// A numeric literal (dimensionless).
    Number(f64),
    /// The constant `pi`.
    Pi,
    /// A parameter or measure reference, possibly dotted (`Body1.height`).
    Ident(String),
    /// Unary minus.
    Neg(Box<Expr>),
    /// A binary operation.
    Binary {
        /// Operator.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// A unit suffix applied to a plain number.
    Unit {
        /// The number (or parenthesised group) being given a unit.
        expr: Box<Expr>,
        /// The unit.
        unit: Unit,
    },
    /// A function call.
    Call {
        /// The function.
        func: Func,
        /// Its arguments.
        args: Vec<Expr>,
    },
}

impl Expr {
    pub(crate) const fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub(crate) fn binary(op: BinOp, lhs: Self, rhs: Self) -> Self {
        let span = lhs.span.merge(rhs.span);
        Self::new(
            ExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            span,
        )
    }

    /// Every identifier this expression reads (never `pi`). Sorted, so deterministic.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_refs(&mut out);
        out
    }

    fn collect_refs(&self, out: &mut BTreeSet<String>) {
        match &self.kind {
            ExprKind::Number(_) | ExprKind::Pi => {}
            ExprKind::Ident(n) => {
                out.insert(n.clone());
            }
            ExprKind::Neg(e) | ExprKind::Unit { expr: e, .. } => e.collect_refs(out),
            ExprKind::Binary { lhs, rhs, .. } => {
                lhs.collect_refs(out);
                rhs.collect_refs(out);
            }
            ExprKind::Call { args, .. } => args.iter().for_each(|a| a.collect_refs(out)),
        }
    }
}

/// Dependency extraction: the identifiers `expr` reads.
pub fn references(expr: &Expr) -> BTreeSet<String> {
    expr.references()
}
