//! Expression errors: a stable code, an English message and a byte span.

use super::quantity::Dim;
use thiserror::Error;

/// A half-open byte range `start..end` into the source text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    /// First byte.
    pub start: usize,
    /// One past the last byte.
    pub end: usize,
}

impl Span {
    /// Creates a span.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Smallest span covering both.
    pub fn merge(self, other: Self) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl core::fmt::Display for Span {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// What went wrong. Messages are English; UI localisation keys off [`ExprErrorKind::code`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ExprErrorKind {
    /// Input exceeds the maximum length.
    #[error("expression is longer than {max} bytes")]
    InputTooLong {
        /// The limit.
        max: usize,
    },
    /// Input has too many tokens.
    #[error("expression has more than {max} tokens")]
    TooManyTokens {
        /// The limit.
        max: usize,
    },
    /// Nesting is too deep.
    #[error("expression is nested deeper than {max} levels")]
    TooDeep {
        /// The limit.
        max: usize,
    },
    /// A character that is not part of the language.
    #[error("unexpected character `{ch}`")]
    UnexpectedChar {
        /// The character.
        ch: char,
    },
    /// A malformed number literal.
    #[error("invalid number")]
    InvalidNumber,
    /// A token that does not fit the grammar.
    #[error("expected {expected}, found {found}")]
    UnexpectedToken {
        /// What the parser wanted.
        expected: &'static str,
        /// What it found.
        found: String,
    },
    /// The input ended too early.
    #[error("expected {expected}, found end of input")]
    UnexpectedEnd {
        /// What the parser wanted.
        expected: &'static str,
    },
    /// A call to a function that does not exist.
    #[error("unknown function `{name}`")]
    UnknownFunction {
        /// The function name.
        name: String,
    },
    /// A call with the wrong number of arguments.
    #[error("`{name}` takes {min}..={max} arguments, got {found}")]
    WrongArgumentCount {
        /// The function name.
        name: &'static str,
        /// Minimum arguments.
        min: usize,
        /// Maximum arguments.
        max: usize,
        /// Arguments given.
        found: usize,
    },
    /// The name on the left of `=name = expr` is not allowed.
    #[error("`{name}` cannot be used as a parameter name")]
    InvalidDefinitionName {
        /// The rejected name.
        name: String,
    },
    /// The input was required to be `name = expr` but is a plain expression.
    #[error("expected a definition of the form `name = expression`")]
    NotADefinition,
    /// An identifier the context could not resolve.
    #[error("unknown identifier `{name}`")]
    UnknownIdentifier {
        /// The identifier.
        name: String,
    },
    /// Operands of `+`, `-`, `min` or `max` have different dimensions.
    #[error("incompatible dimensions: {left} and {right}")]
    DimensionMismatch {
        /// Left (or first) operand.
        left: Dim,
        /// Right (or offending) operand.
        right: Dim,
    },
    /// An operand has a dimension the operation cannot accept.
    #[error("{context} (found {found})")]
    InvalidOperandDimension {
        /// What was required.
        context: &'static str,
        /// The dimension found.
        found: Dim,
    },
    /// Dimension exponents grew beyond +-6.
    #[error("dimension exponent out of range")]
    ExponentOverflow,
    /// Division by zero.
    #[error("division by zero")]
    DivisionByZero,
    /// A result that is NaN or infinite.
    #[error("result is not a finite number")]
    NotFinite,
    /// A value whose magnitude exceeds 1e15.
    #[error("value magnitude exceeds 1e15")]
    OutOfRange,
    /// An argument outside a function's domain.
    #[error("argument outside the domain of `{function}`")]
    DomainError {
        /// The function name.
        function: &'static str,
    },
}

impl ExprErrorKind {
    /// Stable, machine-readable code (never changes once released; used for i18n lookup).
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InputTooLong { .. } => "math.expr.input_too_long",
            Self::TooManyTokens { .. } => "math.expr.too_many_tokens",
            Self::TooDeep { .. } => "math.expr.too_deep",
            Self::UnexpectedChar { .. } => "math.expr.unexpected_char",
            Self::InvalidNumber => "math.expr.invalid_number",
            Self::UnexpectedToken { .. } => "math.expr.unexpected_token",
            Self::UnexpectedEnd { .. } => "math.expr.unexpected_end",
            Self::UnknownFunction { .. } => "math.expr.unknown_function",
            Self::WrongArgumentCount { .. } => "math.expr.wrong_argument_count",
            Self::InvalidDefinitionName { .. } => "math.expr.invalid_definition_name",
            Self::NotADefinition => "math.expr.not_a_definition",
            Self::UnknownIdentifier { .. } => "math.expr.unknown_identifier",
            Self::DimensionMismatch { .. } => "math.expr.dimension_mismatch",
            Self::InvalidOperandDimension { .. } => "math.expr.invalid_operand_dimension",
            Self::ExponentOverflow => "math.expr.exponent_overflow",
            Self::DivisionByZero => "math.expr.division_by_zero",
            Self::NotFinite => "math.expr.not_finite",
            Self::OutOfRange => "math.expr.out_of_range",
            Self::DomainError { .. } => "math.expr.domain_error",
        }
    }
}

/// An expression error with the byte span it refers to.
#[derive(Debug, Clone, PartialEq, Error)]
#[error("{kind} (at bytes {span})")]
pub struct ExprError {
    /// What went wrong.
    pub kind: ExprErrorKind,
    /// Where, as byte offsets into the source.
    pub span: Span,
}

impl ExprError {
    /// Creates an error.
    pub const fn new(kind: ExprErrorKind, span: Span) -> Self {
        Self { kind, span }
    }

    /// Stable error code, see [`ExprErrorKind::code`].
    pub const fn code(&self) -> &'static str {
        self.kind.code()
    }
}
