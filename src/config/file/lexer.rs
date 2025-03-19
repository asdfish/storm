//! Conf file lexer

use {
    std::borrow::Cow,
    unicode_ident::{is_xid_continue, is_xid_start},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lexer<'src> {
    src: &'src str,
    next: &'src str,
    line: usize,
}
impl<'src> Lexer<'src> {
    pub const fn new(src: &'src str) -> Self {
        Self {
            src,
            next: src,
            line: 0,
        }
    }

    fn next_int(
        &mut self,
        next: Option<&'src str>,
        sign: Sign,
    ) -> Option<Result<Token<'src>, LexerError<'src>>> {
        let next = next.unwrap_or(self.next);
        let mut chars = next.char_indices().peekable();

        let (radix, next) = if let Some((_, '0')) = chars.next() {
            match chars.next() {
                Some((index, ch @ 'b')) => Some((Radix::Binary, index + ch.len_utf8())),
                Some((index, ch @ 'o')) => Some((Radix::Octal, index + ch.len_utf8())),
                Some((index, ch @ 'd')) => Some((Radix::Decimal, index + ch.len_utf8())),
                Some((index, ch @ 'x')) => Some((Radix::Hexadecimal, index + ch.len_utf8())),
                _ => None,
            }
        } else {
            None
        }
        .map(|(radix, index)| (radix, &next[index..]))
        .unwrap_or((Radix::Decimal, next));

        self.next_int_with_radix(next, radix, sign)
    }

    fn next_int_with_radix(
        &mut self,
        next: &'src str,
        radix: Radix,
        sign: Sign,
    ) -> Option<Result<Token<'src>, LexerError<'src>>> {
        let (int, next) = match next
            .char_indices()
            .take_while(|(_, ch)| radix.validate(*ch))
            .last()
            .and_then(|(i, ch)| next.split_at_checked(i + ch.len_utf8()))
        {
            Some(bundle) => bundle,
            None => {
                return Some(Err(LexerError::new(
                    *self,
                    LexerErrorKind::Unexpected {
                        expected: radix.digit_rule(),
                        got: Expectation::Eof,
                    },
                )))
            }
        };

        let overflow_error = || LexerError::new(*self, LexerErrorKind::IntOverflow(int));

        match int.chars().try_fold(0_i64, |int, ch| {
            int.checked_mul(radix.into())
                .ok_or_else(overflow_error)?
                .checked_add(
                    ch.to_digit(radix.into())
                        .expect("internal error: [Radix::validate] should ensure that all characters are valid digits in its radix")
                        .into()
                )
                .ok_or_else(overflow_error)
        }) {
            Ok(int) => self.submit_token_with_str(Token::Int(if sign == Sign::Negative {
                -int
            } else {
                int
            }), next),
            Err(err) => Some(Err(err)),
        }
    }

    #[inline]
    fn submit_token_with_str(
        &mut self,
        token: Token<'src>,
        next: &'src str,
    ) -> Option<Result<Token<'src>, LexerError<'src>>> {
        self.next = next;
        if matches!(token, Token::NewLine) {
            self.line += 1;
        }

        Some(Ok(token))
    }
    #[inline]
    fn submit_token_with_iter<I>(
        &mut self,
        token: Token<'src>,
        mut chars: I,
    ) -> Option<Result<Token<'src>, LexerError<'src>>>
    where
        I: Iterator<Item = (usize, char)>,
    {
        self.submit_token_with_str(
            token,
            chars
                .next()
                .map(|(index, _)| index)
                .and_then(|index| self.next.get(index..))
                .unwrap_or_default(),
        )
    }
}
impl<'src> Iterator for Lexer<'src> {
    type Item = Result<Token<'src>, LexerError<'src>>;

    fn next(&mut self) -> Option<Result<Token<'src>, LexerError<'src>>> {
        let mut chars = self.next.char_indices().peekable();
        while chars
            .next_if(|(_, ch)| ch.is_whitespace() && !matches!(ch, '\n' | '\r'))
            .is_some()
        {}

        self.next = chars
            .next()
            .and_then(|(index, _)| self.next.get(index..))
            .unwrap_or_default();
        let mut chars = self.next.char_indices().peekable();

        match chars.next().map(|(_, ch)| ch)? {
            '[' => self.submit_token_with_iter(Token::LBrace, chars),
            ']' => self.submit_token_with_iter(Token::RBrace, chars),
            '=' => self.submit_token_with_iter(Token::Assign, chars),
            ',' => self.submit_token_with_iter(Token::Comma, chars),
            '\n' => self.submit_token_with_iter(Token::NewLine, chars),
            '\r' => match chars.next() {
                Some((_, '\n')) => self.submit_token_with_iter(Token::NewLine, chars),
                Some((_, ch)) => Some(Err(LexerError::new(
                    *self,
                    LexerErrorKind::Unexpected {
                        expected: Expectation::Regex("\\n"),
                        got: Expectation::Char(ch),
                    },
                ))),
                None => Some(Err(LexerError::new(
                    *self,
                    LexerErrorKind::Unexpected {
                        expected: Expectation::Regex("\\n"),
                        got: Expectation::Eof,
                    },
                ))),
            },
            '-' => self.next_int(Some(&self.next['-'.len_utf8()..]), Sign::Negative),
            '+' => self.next_int(Some(&self.next['+'.len_utf8()..]), Sign::Positive),
            '0'..='9' => self.next_int(None, Sign::Positive),
            ch if is_xid_start(ch) => {
                let (ident, next) = self.next.split_at(
                    chars
                        .take_while(|(_, ch)| is_xid_continue(*ch))
                        .last()
                        .map(|(i, ch)| i + ch.len_utf8())
                        .unwrap_or(ch.len_utf8()),
                );

                self.submit_token_with_str(
                    match ident {
                        "true" => Token::Bool(true),
                        "false" => Token::Bool(false),
                        _ => Token::Ident(ident),
                    },
                    next,
                )
            }
            '"' => {
                self.next = chars
                    .next()
                    .and_then(|(index, _)| self.next.get(index..))
                    .unwrap_or_default();
                let mut chars = self.next.char_indices();

                let mut string = Cow::Borrowed("");

                let end = loop {
                    // println!("{}", chars.as_str());

                    match chars.next() {
                        Some((_, '\\')) => string.to_mut().push(match chars.next() {
                            Some((_, 'n')) => '\n',
                            Some((_, 'r')) => '\r',
                            Some((_, 't')) => '\t',
                            Some((_, '"')) => '"',
                            Some((_, '\'')) => '\'',
                            Some((_, '\\')) => '\\',
                            Some((_, ch)) => {
                                return Some(Err(LexerError::new(
                                    *self,
                                    LexerErrorKind::Unexpected {
                                        expected: Expectation::Regex(r#"[nrt"\\]"#),
                                        got: Expectation::Char(ch),
                                    },
                                )))
                            }
                            None => {
                                return Some(Err(LexerError::new(
                                    *self,
                                    LexerErrorKind::Unexpected {
                                        expected: Expectation::Regex(r#"[nrt"\\]"#),
                                        got: Expectation::Eof,
                                    },
                                )))
                            }
                        }),
                        Some((index, ch @ '"')) => {
                            break index + ch.len_utf8();
                        },
                        Some((end, ch)) => {
                            match &mut string {
                                Cow::Owned(string) => {
                                    string.push(ch);
                                }
                                Cow::Borrowed(_) => {
                                    string = Cow::Borrowed(&self.next[..end + ch.len_utf8()]);
                                }
                            }
                        }
                        None => {
                            return Some(Err(LexerError::new(
                                *self,
                                LexerErrorKind::Unexpected {
                                    expected: Expectation::Regex(r#"[^\\"]|(\\[nrt"\\])"#),
                                    got: Expectation::Eof,
                                },
                            )))
                        }
                    }
                };

                self.submit_token_with_str(Token::String(string), &self.next[end..])
            },
            ch => Some(Err(LexerError::new(
                *self,
                LexerErrorKind::Unexpected {
                    expected: Expectation::Regex(r#"[\[\]=,\n\r-+0-9\p{XID_Start}"]"#),
                    got: Expectation::Char(ch),
                },
            ))),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
    NewLine,
    LBrace,
    RBrace,
    Assign,
    Comma,
    Bool(bool),
    Int(i64),
    Ident(&'src str),
    String(Cow<'src, str>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LexerError<'src> {
    state: Lexer<'src>,
    kind: LexerErrorKind<'src>,
}
impl<'src> LexerError<'src> {
    const fn new(state: Lexer<'src>, kind: LexerErrorKind<'src>) -> Self {
        Self { state, kind }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum LexerErrorKind<'src> {
    IntOverflow(&'src str),
    Unexpected {
        expected: Expectation,
        got: Expectation,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Expectation {
    Eof,
    Char(char),
    Regex(&'static str),
}
impl From<char> for Expectation {
    fn from(ch: char) -> Self {
        Self::Char(ch)
    }
}
impl From<&'static str> for Expectation {
    fn from(regex: &'static str) -> Self {
        Self::Regex(regex)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Sign {
    #[default]
    Positive,
    Negative,
}

#[derive(Clone, Copy, Debug, Default)]
enum Radix {
    Binary,
    Octal,
    #[default]
    Decimal,
    Hexadecimal,
}
impl Radix {
    /// Return the rule for the digits *not* the rule, which does not include the radix head.
    pub const fn digit_rule(&self) -> Expectation {
        match self {
            Self::Binary => Expectation::Regex("[01]+"),
            Self::Octal => Expectation::Regex("[0-7]+"),
            Self::Decimal => Expectation::Regex("\\d+"),
            Self::Hexadecimal => Expectation::Regex("[\\da-fA-F]+"),
        }
    }
    pub fn validate(&self, ch: char) -> bool {
        ch.is_digit(u32::from(*self))
    }
}
macro_rules! impl_from_radix_for_number {
    ($ty:ty) => {
        impl From<Radix> for $ty {
            fn from(radix: Radix) -> $ty {
                match radix {
                    Radix::Binary => 2,
                    Radix::Octal => 8,
                    Radix::Decimal => 10,
                    Radix::Hexadecimal => 16,
                }
            }
        }
    };
}
impl_from_radix_for_number!(i64);
impl_from_radix_for_number!(u32);

#[cfg(test)]
mod tests {
    use super::*;

    fn token_list<const N: usize>(tokens: [(&'static str, Token<'static>); N]) {
        // test concat
        {
            let src = tokens
                .iter()
                .flat_map(|(token, _)| [token, " "])
                .collect::<String>();

            let mut lexer = Lexer::new(&src);
            tokens
                .iter()
                .map(|(_, expectation)| {
                    (lexer.next().unwrap(), expectation)
                })
                .for_each(|(token, expectation)| assert_eq!(token.as_ref(), Ok(expectation)));
            assert_eq!(lexer.next(), None);
        }

        // test singular
        tokens.into_iter().for_each(|(src, expectation)| {
            let mut lexer = Lexer::new(src);
            assert_eq!(lexer.next(), Some(Ok(expectation)));
            assert_eq!(lexer.next(), None);
        });
    }

    #[test]
    fn static_inputs() {
        token_list([
            ("[", Token::LBrace),
            (",", Token::Comma),
            ("]", Token::RBrace),
            ("=", Token::Assign),
            ("\n", Token::NewLine),
            ("\r\n", Token::NewLine),
        ]);
    }

    #[test]
    fn fauly_inputs() {
        ["0b", "0o", "0d", "0x", "\r", r#""\a""#, r#"""#, r#""\""#]
            .into_iter()
            .map(Lexer::new)
            .map(|mut lexer| lexer.next())
            .map(Option::unwrap)
            .map(Result::unwrap_err)
            .for_each(drop);
    }

    #[test]
    fn varadic_inputs() {
        token_list([
            ("true", Token::Bool(true)),
            ("false", Token::Bool(false)),
            ("foo", Token::Ident("foo")),
            ("bar", Token::Ident("bar")),
            ("0", Token::Int(0)),
            ("0b10", Token::Int(0b10)),
            ("0o12345670", Token::Int(0o12345670)),
            ("0d1234567890", Token::Int(1234567890)),
            ("0x123456789abcdef0", Token::Int(0x123456789abcdef0)),
            ("+0", Token::Int(0)),
            ("+0b10", Token::Int(0b10)),
            ("+0o12345670", Token::Int(0o12345670)),
            ("+0d1234567890", Token::Int(1234567890)),
            ("+0x123456789abcdef0", Token::Int(0x123456789abcdef0)),
            ("-0", Token::Int(-0)),
            ("-0b10", Token::Int(-0b10)),
            ("-0o12345670", Token::Int(-0o12345670)),
            ("-0d1234567890", Token::Int(-1234567890)),
            ("-0x123456789abcdef0", Token::Int(-0x123456789abcdef0)),
            (r#""hello world\n\r\t\"\'""#, Token::String("hello world\n\r\t\"\'".into()))
        ]);
    }
}
