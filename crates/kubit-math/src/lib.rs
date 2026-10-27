//! Units, tolerances and parameter expressions (L0).
//!
//! Layer and rules: `docs/ARCHITECTURE.md`.
//!
//! Everything here works internally in **millimetres and radians** (`f64`). Other units exist
//! only at the edges: parsing user input and formatting for display.
//!
//! * [`units`]: [`Length`] / [`Angle`] newtypes, unit conversion, display formatting.
//! * [`tolerance`]: modelling tolerances and approximate comparisons.
//! * [`expr`]: the parameter expression language (`=ancho/2 + 3 mm`, `3/4 in`, `=alto = 100 mm`).

pub mod expr;
pub mod tolerance;
pub mod units;

pub use expr::{
    Context, Definition, Dim, Expr, ExprError, ExprErrorKind, Input, ParameterSet, Quantity, Span,
    evaluate, parse, parse_definition, parse_input,
};
pub use tolerance::Tolerance;
pub use units::{Angle, AngleUnit, Length, LengthUnit, Unit, UnitError};
