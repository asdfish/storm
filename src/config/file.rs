pub mod lexer;

use {
    lexer::{Lexer, LexerError, Literal, Token},
    smallvec::SmallVec,
    std::iter::Peekable,
};

#[derive(Debug, PartialEq)]
pub enum Instruction<'src> {
    Array(&'src str, SmallVec<[Literal<'src>; 8]>),
    Literal(&'src str, Literal<'src>),
}

pub struct Parser<'src>(Peekable<Lexer<'src>>);
impl<'src> Parser<'src> {
    pub fn new(src: &'src str) -> Self {
        Self(Lexer::new(src).peekable())
    }
}
impl<'src> Iterator for Parser<'src> {
    type Item = Result<Instruction<'src>, ParserError<'src>>;

    fn next(&mut self) -> Option<Result<Instruction<'src>, ParserError<'src>>> {
        macro_rules! next {
            () => {
                match self.0.next()? {
                    Ok(token) => token,
                    Err(err) => return Some(Err(err.into())),
                }
            };
            ($lexer:expr) => {
                match $lexer.next()? {
                    Ok(token) => token,
                    Err(err) => return Some(Err(err.into())),
                }
            };
        }
        macro_rules! assert_next_token {
            ($pat:pat, $ty:expr) => {
                match next!() {
                    $pat => {}
                    token => {
                        return Some(Err(ParserError::Unexpected {
                            expected: $ty,
                            got: token.into(),
                        }))
                    }
                }
            };
        }
        macro_rules! newline_or_eof {
            () => {
                match self.0.next().transpose() {
                    Ok(token) => match token {
                        Some(Token::NewLine) | None => {}
                        Some(token) => {
                            return Some(Err(ParserError::Unexpected {
                                expected: TokenTy::Choice(&[TokenTy::Eof, TokenTy::NewLine]),
                                got: token.into(),
                            }))
                        }
                    },
                    Err(err) => return Some(Err(err.into())),
                }
            }
        }

        match next!() {
            Token::Ident(ident) => {
                assert_next_token!(Token::Assign, TokenTy::Assign);
                let instruction = match next!() {
                    Token::Literal(literal) => Instruction::Literal(ident, literal),
                    Token::LBrace => {
                        let mut items = SmallVec::new();

                        let mut lexer = self.0.by_ref()
                            .filter(|token| token.as_ref().map(|token| *token != Token::NewLine).unwrap_or(true));

                        loop {
                            match next!(lexer) {
                                Token::Literal(item) => {
                                    items.push(item);

                                    match next!(lexer) {
                                        Token::Comma => continue,
                                        Token::RBrace => break,
                                        token => return Some(Err(ParserError::Unexpected {
                                            expected: TokenTy::Choice(&[TokenTy::Comma, TokenTy::RBrace]),
                                            got: token.into(),
                                        })),
                                    }
                                }
                                Token::RBrace => break,
                                token => return Some(Err(ParserError::Unexpected {
                                    expected: TokenTy::Choice(&[TokenTy::Comma, TokenTy::RBrace]),
                                    got: token.into(),
                                })),
                            }
                        }
                        newline_or_eof!();

                        Instruction::Array(ident, items)
                    },
                    token => return Some(Err(ParserError::Unexpected {
                        expected: TokenTy::Choice(&[TokenTy::RBrace, TokenTy::Literal]),
                        got: token.into(),
                    })),
                };
                newline_or_eof!();

                Some(Ok(instruction))
            }
            token => Some(Err(ParserError::Unexpected {
                expected: TokenTy::Choice(&[TokenTy::LBrace]),
                got: token.into(),
            })),
        }
        // match advance!(next) {
        //     Token::LBrace => {
        //         assert_token!(next, Token::RBrace, TokenTy::RBrace);
        //         assert_token!(peek, Token::NewLine, TokenTy::NewLine);

        //         Some(Ok(Instruction::ChangeSection(ident)))
        //     }
        //     _ => todo!()
        // }
    }
}

#[derive(Debug, PartialEq)]
pub enum ParserError<'src> {
    Lexer(LexerError<'src>),
    Unexpected { expected: TokenTy, got: TokenTy },
}
impl<'src> From<LexerError<'src>> for ParserError<'src> {
    fn from(err: LexerError<'src>) -> Self {
        Self::Lexer(err)
    }
}

#[derive(Debug, PartialEq)]
pub enum TokenTy {
    Assign,
    Comma,
    Eof,
    Ident,
    LBrace,
    Literal,
    NewLine,
    RBrace,

    Choice(&'static [Self]),
}
impl<'src> From<Token<'src>> for TokenTy {
    fn from(token: Token<'src>) -> Self {
        match token {
            Token::Assign => Self::Assign,
            Token::Comma => Self::Comma,
            Token::Ident(_) => Self::Ident,
            Token::LBrace => Self::LBrace,
            Token::Literal(_) => Self::Literal,
            Token::NewLine => Self::NewLine,
            Token::RBrace => Self::RBrace,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal() {
        let mut parser = Parser::new("foo = \"bar\"\nbar = [\n1,\n0xDEADBEEF,\n\n\n]");

        assert_eq!(parser.next().unwrap().unwrap(), Instruction::Literal("foo", Literal::String("bar".into())));
        assert_eq!(parser.next().unwrap().unwrap(), Instruction::Array("bar", vec![
            Literal::Int(1),
            Literal::Int(0xDEADBEEF),
        ].into()));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn faulty_inputs() {
        [
            "foo",
            "[]",
            "[foo",
            "foo=",
            "0xDEADBEEF=10",
        ]
            .into_iter()
            .for_each(|input| assert!(Parser::new(input).next().transpose().ok().flatten().is_none()));
    }
}
