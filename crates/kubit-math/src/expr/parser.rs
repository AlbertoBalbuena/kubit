//! Recursive-descent parser.
//!
//! ```text
//! expr    := term (('+' | '-') term)*
//! term    := unary (('*' | '/') unary)*
//! unary   := ('-' | '+') unary | power
//! power   := primary ('^' exponent)?
//! exponent:= ('-' | '+') exponent | power          (right associative)
//! primary := NUMBER [NUMBER '/' NUMBER] [UNIT]     mixed number: 1 1/2 in
//!          | NUMBER '/' NUMBER UNIT                fraction of a unit: 3/4 in
//!          | IDENT '(' [expr (',' expr)*] ')' | IDENT | 'pi'
//!          | '(' expr ')' [UNIT]
//! ```

use super::ast::{BinOp, Expr, ExprKind, Func};
use super::error::{ExprError, ExprErrorKind, Span};
use super::lexer::{Tok, Token, lex};
use super::{MAX_CALL_ARGS, MAX_NESTING_DEPTH};
use crate::units::{AngleUnit, LengthUnit, Unit};

type R<T> = Result<T, ExprError>;

/// `name = expression`, the inline definition syntax (`=alto_zoclo = 100 mm`).
#[derive(Debug, Clone, PartialEq)]
pub struct Definition {
    /// Parameter name (a plain identifier, no dots).
    pub name: String,
    /// Where the name is in the source.
    pub name_span: Span,
    /// The defining expression.
    pub expr: Expr,
}

/// A parsed inline input field.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// A plain expression (`=ancho/2`, `3/4 in`).
    Expression(Expr),
    /// A parameter definition (`=alto = 100 mm`).
    Definition(Definition),
}

/// Parses an expression. A single leading `=` (the inline-input marker) is accepted and ignored.
pub fn parse(src: &str) -> R<Expr> {
    let toks = lex(src)?;
    let mut p = Parser::new(&toks, src.len());
    p.skip_equals();
    p.expr_to_end()
}

/// Parses an inline input field: an expression or a `name = expression` definition, with an
/// optional leading `=`.
pub fn parse_input(src: &str) -> R<Input> {
    let toks = lex(src)?;
    let mut p = Parser::new(&toks, src.len());
    p.skip_equals();
    if p.looks_like_definition() {
        p.definition().map(Input::Definition)
    } else {
        p.expr_to_end().map(Input::Expression)
    }
}

/// Parses `name = expression` (optional leading `=`); anything else is an error.
pub fn parse_definition(src: &str) -> R<Definition> {
    match parse_input(src)? {
        Input::Definition(d) => Ok(d),
        Input::Expression(_) => Err(ExprError::new(
            ExprErrorKind::NotADefinition,
            Span::new(0, src.len()),
        )),
    }
}

fn unit_of(tok: &Tok) -> Option<Unit> {
    match tok {
        Tok::Ident(s) => Unit::from_symbol(s),
        Tok::Quote => Some(Unit::Length(LengthUnit::Inch)),
        Tok::Degree => Some(Unit::Angle(AngleUnit::Degree)),
        _ => None,
    }
}

fn describe(tok: &Tok) -> String {
    match tok {
        Tok::Number { .. } => "a number".into(),
        Tok::Ident(n) => format!("identifier `{n}`"),
        Tok::Plus => "`+`".into(),
        Tok::Minus => "`-`".into(),
        Tok::Star => "`*`".into(),
        Tok::Slash => "`/`".into(),
        Tok::Caret => "`^`".into(),
        Tok::LParen => "`(`".into(),
        Tok::RParen => "`)`".into(),
        Tok::Comma => "`,`".into(),
        Tok::Equals => "`=`".into(),
        Tok::Quote => "`\"`".into(),
        Tok::Degree => "`°`".into(),
    }
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    depth: usize,
    src_len: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token], src_len: usize) -> Self {
        Self {
            toks,
            pos: 0,
            depth: 0,
            src_len,
        }
    }

    fn token(&self, offset: usize) -> Option<&'a Token> {
        let toks: &'a [Token] = self.toks;
        toks.get(self.pos + offset)
    }

    fn tok(&self, offset: usize) -> Option<&'a Tok> {
        self.token(offset).map(|t| &t.tok)
    }

    fn unexpected(&self, expected: &'static str) -> ExprError {
        match self.token(0) {
            Some(t) => ExprError::new(
                ExprErrorKind::UnexpectedToken {
                    expected,
                    found: describe(&t.tok),
                },
                t.span,
            ),
            None => ExprError::new(
                ExprErrorKind::UnexpectedEnd { expected },
                Span::new(self.src_len, self.src_len),
            ),
        }
    }

    fn expect(&mut self, want: &Tok, name: &'static str) -> R<Span> {
        match self.token(0) {
            Some(t) if t.tok == *want => {
                self.pos += 1;
                Ok(t.span)
            }
            _ => Err(self.unexpected(name)),
        }
    }

    fn enter(&mut self, span: Span) -> R<()> {
        self.depth += 1;
        if self.depth > MAX_NESTING_DEPTH {
            return Err(ExprError::new(
                ExprErrorKind::TooDeep {
                    max: MAX_NESTING_DEPTH,
                },
                span,
            ));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn skip_equals(&mut self) {
        if self.tok(0) == Some(&Tok::Equals) {
            self.pos += 1;
        }
    }

    fn looks_like_definition(&self) -> bool {
        matches!(self.tok(0), Some(Tok::Ident(_))) && self.tok(1) == Some(&Tok::Equals)
    }

    fn definition(&mut self) -> R<Definition> {
        let (
            Some(Token {
                tok: Tok::Ident(name),
                span,
            }),
            true,
        ) = (self.token(0), self.tok(1).is_some())
        else {
            return Err(self.unexpected("a parameter name"));
        };
        if name.contains('.') || name == "pi" || Func::from_name(name).is_some() {
            return Err(ExprError::new(
                ExprErrorKind::InvalidDefinitionName { name: name.clone() },
                *span,
            ));
        }
        self.pos += 2;
        let expr = self.expr_to_end()?;
        Ok(Definition {
            name: name.clone(),
            name_span: *span,
            expr,
        })
    }

    fn expr_to_end(&mut self) -> R<Expr> {
        let e = self.expr()?;
        if self.pos < self.toks.len() {
            return Err(self.unexpected("end of input"));
        }
        Ok(e)
    }

    fn expr(&mut self) -> R<Expr> {
        let span = self
            .token(0)
            .map_or(Span::new(self.src_len, self.src_len), |t| t.span);
        self.enter(span)?;
        let r = self.additive();
        self.leave();
        r
    }

    fn additive(&mut self) -> R<Expr> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.tok(0) {
                Some(Tok::Plus) => BinOp::Add,
                Some(Tok::Minus) => BinOp::Sub,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            let rhs = self.term()?;
            lhs = Expr::binary(op, lhs, rhs);
        }
    }

    fn term(&mut self) -> R<Expr> {
        let mut lhs = self.unary(true)?;
        loop {
            let op = match self.tok(0) {
                Some(Tok::Star) => BinOp::Mul,
                Some(Tok::Slash) => BinOp::Div,
                _ => return Ok(lhs),
            };
            self.pos += 1;
            let rhs = self.unary(true)?;
            lhs = Expr::binary(op, lhs, rhs);
        }
    }

    /// `allow_frac` enables the `n/d unit` fraction form; it is off for exponent operands, so in
    /// `2^3/4 in` the `3` is not a numerator and the result is `(2^3) / (4 in)`.
    fn unary(&mut self, allow_frac: bool) -> R<Expr> {
        let Some(&Token { ref tok, span }) = self.token(0) else {
            return self.power(allow_frac);
        };
        match tok {
            Tok::Minus => {
                self.pos += 1;
                self.enter(span)?;
                let inner = self.unary(allow_frac);
                self.leave();
                let inner = inner?;
                let s = span.merge(inner.span);
                Ok(Expr::new(ExprKind::Neg(Box::new(inner)), s))
            }
            Tok::Plus => {
                self.pos += 1;
                self.enter(span)?;
                let inner = self.unary(allow_frac);
                self.leave();
                inner
            }
            _ => self.power(allow_frac),
        }
    }

    fn power(&mut self, allow_frac: bool) -> R<Expr> {
        let base = self.primary(allow_frac)?;
        if self.tok(0) != Some(&Tok::Caret) {
            return Ok(base);
        }
        self.pos += 1;
        let exp = self.exponent()?;
        Ok(Expr::binary(BinOp::Pow, base, exp))
    }

    fn exponent(&mut self) -> R<Expr> {
        let span = self
            .token(0)
            .map_or(Span::new(self.src_len, self.src_len), |t| t.span);
        self.enter(span)?;
        let r = self.exponent_inner(span);
        self.leave();
        r
    }

    fn exponent_inner(&mut self, span: Span) -> R<Expr> {
        match self.tok(0) {
            Some(Tok::Minus) => {
                self.pos += 1;
                let inner = self.exponent()?;
                let s = span.merge(inner.span);
                Ok(Expr::new(ExprKind::Neg(Box::new(inner)), s))
            }
            Some(Tok::Plus) => {
                self.pos += 1;
                self.exponent()
            }
            _ => self.power(false),
        }
    }

    fn unit_suffix(&mut self, node: Expr) -> Expr {
        match self
            .token(0)
            .and_then(|t| unit_of(&t.tok).map(|u| (u, t.span)))
        {
            Some((unit, uspan)) => {
                self.pos += 1;
                let span = node.span.merge(uspan);
                Expr::new(
                    ExprKind::Unit {
                        expr: Box::new(node),
                        unit,
                    },
                    span,
                )
            }
            None => node,
        }
    }

    fn primary(&mut self, allow_frac: bool) -> R<Expr> {
        let Some(&Token { ref tok, span }) = self.token(0) else {
            return Err(self.unexpected("an expression"));
        };
        match tok {
            Tok::Number { value, integer } => {
                self.pos += 1;
                self.number_tail(*value, *integer, span, allow_frac)
            }
            Tok::Ident(name) => {
                self.pos += 1;
                if self.tok(0) == Some(&Tok::LParen) {
                    self.call(name, span)
                } else if name == "pi" {
                    Ok(Expr::new(ExprKind::Pi, span))
                } else {
                    Ok(Expr::new(ExprKind::Ident(name.clone()), span))
                }
            }
            Tok::LParen => {
                self.pos += 1;
                let inner = self.expr()?;
                let close = self.expect(&Tok::RParen, "`)`")?;
                let mut inner = inner;
                inner.span = span.merge(close);
                Ok(self.unit_suffix(inner))
            }
            _ => Err(self.unexpected("an expression")),
        }
    }

    fn number_tail(&mut self, value: f64, integer: bool, span: Span, allow_frac: bool) -> R<Expr> {
        let mut node = Expr::new(ExprKind::Number(value), span);
        if integer {
            let ints = (
                self.token(0).map(|t| &t.tok),
                self.tok(1),
                self.token(2).map(|t| &t.tok),
            );
            if let (
                Some(Tok::Number {
                    value: n,
                    integer: true,
                }),
                Some(Tok::Slash),
                Some(Tok::Number {
                    value: d,
                    integer: true,
                }),
            ) = ints
            {
                // Mixed number: `1 1/2`.
                let nspan = self.token(0).map_or(span, |t| t.span);
                let dspan = self.token(2).map_or(span, |t| t.span);
                self.pos += 3;
                let frac = Expr::binary(
                    BinOp::Div,
                    Expr::new(ExprKind::Number(*n), nspan),
                    Expr::new(ExprKind::Number(*d), dspan),
                );
                node = Expr::binary(BinOp::Add, node, frac);
            } else if let (
                true,
                Some(Tok::Slash),
                Some(Tok::Number {
                    value: d,
                    integer: true,
                }),
                Some(next),
            ) = (allow_frac, self.tok(0), self.tok(1), self.tok(2))
                && unit_of(next).is_some()
            {
                // Fraction of a unit: `3/4 in`.
                let dspan = self.token(1).map_or(span, |t| t.span);
                self.pos += 2;
                node = Expr::binary(BinOp::Div, node, Expr::new(ExprKind::Number(*d), dspan));
            }
        }
        Ok(self.unit_suffix(node))
    }

    fn call(&mut self, name: &str, name_span: Span) -> R<Expr> {
        let Some(func) = Func::from_name(name) else {
            return Err(ExprError::new(
                ExprErrorKind::UnknownFunction {
                    name: name.to_owned(),
                },
                name_span,
            ));
        };
        self.pos += 1; // `(`
        let mut args = Vec::new();
        if self.tok(0) != Some(&Tok::RParen) {
            loop {
                args.push(self.expr()?);
                if args.len() > MAX_CALL_ARGS {
                    break;
                }
                if self.tok(0) == Some(&Tok::Comma) {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        let close = self.expect(&Tok::RParen, "`,` or `)`")?;
        let span = name_span.merge(close);
        let (min, max) = func.arity();
        if args.len() < min || args.len() > max {
            return Err(ExprError::new(
                ExprErrorKind::WrongArgumentCount {
                    name: func.name(),
                    min,
                    max,
                    found: args.len(),
                },
                span,
            ));
        }
        Ok(Expr::new(ExprKind::Call { func, args }, span))
    }
}
