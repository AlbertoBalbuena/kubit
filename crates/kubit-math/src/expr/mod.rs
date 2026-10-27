//! Parameter expression language.
//!
//! Own recursive-descent parser (no external expression crate: `evalexpr` >= 13 is AGPL and none
//! of the alternatives do dimensional analysis). Supports numbers, `+ - * / ^`, unary minus,
//! parentheses, unit suffixes (`mm cm m in " ft deg rad °`), fractions (`3/4 in`), mixed numbers
//! (`1 1/2 in`), identifiers (`ancho`, `Body1.height`), the functions `sin cos tan sqrt abs min
//! max round floor ceil` and the constant `pi`.
//!
//! # Semantics worth knowing
//!
//! * Values are [`Quantity`]s in mm / rad with a runtime [`Dim`]. `ancho/2 + 3 mm` is fine,
//!   `3 mm + 2 deg` is a dimension error.
//! * A unit suffix binds to the number (or parenthesised group) before it, and needs a plain
//!   number: `(1+2) mm` is 3 mm, `ancho mm` is an error. `n/d unit` with integer `n`, `d` is a
//!   fraction of the unit (`3/4 in` = 0.75 in), not `3 / (4 in)`; this does not apply to an
//!   exponent operand (`2^3/4 in` is `8 / (4 in)`).
//! * `^` is right-associative and binds tighter than unary minus (`-2^2` = -4). A dimensioned
//!   base needs an integer exponent in `-6..=6`.
//! * `round floor ceil` act on the canonical value (mm / rad).
//! * Exponent notation (`1e3`) is not supported.
//! * All intermediate values must be finite with magnitude <= [`MAX_MAGNITUDE`].
//!
//! # Hostile input
//!
//! Input length, token count and nesting depth are capped ([`MAX_INPUT_LEN`], [`MAX_TOKENS`],
//! [`MAX_NESTING_DEPTH`]). Parser recursion is bounded by the nesting cap; the resulting tree
//! is at most [`MAX_TOKENS`] deep, so evaluation and traversal recursion is bounded too.

mod ast;
mod error;
mod eval;
mod lexer;
mod params;
mod parser;
mod quantity;

pub use ast::{BinOp, Expr, ExprKind, Func, references};
pub use error::{ExprError, ExprErrorKind, Span};
pub use eval::{Context, EmptyContext, evaluate};
pub use params::{CycleError, ParamError, ParameterSet};
pub use parser::{Definition, Input, parse, parse_definition, parse_input};
pub use quantity::{Dim, Quantity, QuantityError};

/// Maximum input length in bytes.
pub const MAX_INPUT_LEN: usize = 1024;
/// Maximum number of tokens.
pub const MAX_TOKENS: usize = 512;
/// Maximum nesting depth of parentheses, unary operators, exponents and call arguments.
pub const MAX_NESTING_DEPTH: usize = 64;
/// Maximum number of arguments of a function call.
pub const MAX_CALL_ARGS: usize = 16;
/// Largest magnitude any literal or intermediate value may have.
pub const MAX_MAGNITUDE: f64 = 1.0e15;
/// Largest absolute value of a dimension exponent.
pub const MAX_DIM_EXPONENT: i8 = 6;

/// Formats a number so that [`parse`] reads back exactly the same value.
///
/// Returns `None` for non-finite values or magnitudes above [`MAX_MAGNITUDE`] (which the
/// parser would reject).
pub fn format_number(v: f64) -> Option<String> {
    (v.is_finite() && v.abs() <= MAX_MAGNITUDE).then(|| format!("{v}"))
}
