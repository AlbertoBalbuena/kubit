//! Tokeniser with hard limits.

use super::error::{ExprError, ExprErrorKind, Span};
use super::{MAX_INPUT_LEN, MAX_MAGNITUDE, MAX_TOKENS};
use core::iter::Peekable;
use core::str::CharIndices;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Tok {
    Number { value: f64, integer: bool },
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    Equals,
    Quote,
    Degree,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub tok: Tok,
    pub span: Span,
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn char_at(src: &str, idx: usize) -> Option<char> {
    src.get(idx..).and_then(|s| s.chars().next())
}

type Chars<'a> = Peekable<CharIndices<'a>>;

pub(crate) fn lex(src: &str) -> Result<Vec<Token>, ExprError> {
    if src.len() > MAX_INPUT_LEN {
        return Err(ExprError::new(
            ExprErrorKind::InputTooLong { max: MAX_INPUT_LEN },
            Span::new(0, src.len()),
        ));
    }
    let mut out = Vec::new();
    let mut chars = src.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if out.len() >= MAX_TOKENS {
            return Err(ExprError::new(
                ExprErrorKind::TooManyTokens { max: MAX_TOKENS },
                Span::new(start, start),
            ));
        }
        let (tok, end) = if c.is_ascii_digit()
            || (c == '.' && char_at(src, start + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            number(src, &mut chars, start)?
        } else if is_ident_start(c) {
            ident(src, &mut chars, start)
        } else {
            chars.next();
            let tok = match c {
                '+' => Tok::Plus,
                '-' => Tok::Minus,
                '*' => Tok::Star,
                '/' => Tok::Slash,
                '^' => Tok::Caret,
                '(' => Tok::LParen,
                ')' => Tok::RParen,
                ',' => Tok::Comma,
                '=' => Tok::Equals,
                '"' => Tok::Quote,
                '°' => Tok::Degree,
                ch => {
                    return Err(ExprError::new(
                        ExprErrorKind::UnexpectedChar { ch },
                        Span::new(start, start + ch.len_utf8()),
                    ));
                }
            };
            (tok, start + c.len_utf8())
        };
        out.push(Token {
            tok,
            span: Span::new(start, end),
        });
    }
    Ok(out)
}

fn digits(chars: &mut Chars<'_>, end: &mut usize) {
    while let Some(&(i, c)) = chars.peek() {
        if !c.is_ascii_digit() {
            break;
        }
        *end = i + 1;
        chars.next();
    }
}

fn number(src: &str, chars: &mut Chars<'_>, start: usize) -> Result<(Tok, usize), ExprError> {
    let mut end = start;
    let mut integer = true;
    digits(chars, &mut end);
    if let Some(&(i, '.')) = chars.peek() {
        integer = false;
        end = i + 1;
        chars.next();
        digits(chars, &mut end);
    }
    let span = Span::new(start, end);
    let value: f64 = src
        .get(start..end)
        .and_then(|t| t.parse().ok())
        .ok_or(ExprError::new(ExprErrorKind::InvalidNumber, span))?;
    if !value.is_finite() || value > MAX_MAGNITUDE {
        return Err(ExprError::new(ExprErrorKind::OutOfRange, span));
    }
    Ok((Tok::Number { value, integer }, end))
}

fn ident(src: &str, chars: &mut Chars<'_>, start: usize) -> (Tok, usize) {
    let mut end = start;
    loop {
        while let Some(&(i, c)) = chars.peek() {
            if !is_ident_continue(c) {
                break;
            }
            end = i + c.len_utf8();
            chars.next();
        }
        // A dot continues the name only when another name segment follows (`Body1.height`).
        match chars.peek() {
            Some(&(i, '.')) if char_at(src, i + 1).is_some_and(is_ident_start) => {
                end = i + 1;
                chars.next();
            }
            _ => break,
        }
    }
    let name = src.get(start..end).unwrap_or_default().to_owned();
    (Tok::Ident(name), end)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn numbers() {
        assert_eq!(
            toks("12"),
            [Tok::Number {
                value: 12.0,
                integer: true
            }]
        );
        assert_eq!(
            toks("1.5"),
            [Tok::Number {
                value: 1.5,
                integer: false
            }]
        );
        assert_eq!(
            toks(".5"),
            [Tok::Number {
                value: 0.5,
                integer: false
            }]
        );
        assert_eq!(
            toks("5."),
            [Tok::Number {
                value: 5.0,
                integer: false
            }]
        );
    }

    #[test]
    fn dotted_identifiers() {
        assert_eq!(toks("Body1.height"), [Tok::Ident("Body1.height".into())]);
        assert_eq!(toks("a.b.c"), [Tok::Ident("a.b.c".into())]);
        assert_eq!(toks("áng_1"), [Tok::Ident("áng_1".into())]);
        // A trailing or numeric dot is not part of the name.
        assert_eq!(
            toks("a.5"),
            [
                Tok::Ident("a".into()),
                Tok::Number {
                    value: 0.5,
                    integer: false
                }
            ]
        );
    }

    #[test]
    fn spans_are_byte_offsets() {
        let t = lex("°  é1").unwrap();
        assert_eq!(t[0].span, Span::new(0, 2));
        assert_eq!(t[1].span, Span::new(4, 7));
    }

    #[test]
    fn rejects_unknown_chars_with_span() {
        let e = lex("1 + $").unwrap_err();
        assert_eq!(e.kind, ExprErrorKind::UnexpectedChar { ch: '$' });
        assert_eq!(e.span, Span::new(4, 5));
        assert_eq!(e.code(), "math.expr.unexpected_char");
    }

    #[test]
    fn enforces_limits() {
        let long = "1".repeat(MAX_INPUT_LEN + 1);
        assert_eq!(lex(&long).unwrap_err().code(), "math.expr.input_too_long");
        let many = "1+".repeat(MAX_TOKENS);
        assert_eq!(lex(&many).unwrap_err().code(), "math.expr.too_many_tokens");
        assert_eq!(
            lex("99999999999999999").unwrap_err().code(),
            "math.expr.out_of_range"
        );
        assert!(lex(&"1+".repeat(MAX_TOKENS / 2)).is_ok());
    }
}
