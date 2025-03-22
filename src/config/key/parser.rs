use {
    super::{InvisibleKey, Key, KeyKind, KeyModifier, KeyModifiers, KeySequence},
    std::{borrow::Cow, str},
};

macro_rules! impl_parsable_for {
    ($output:ty, $parser:ty) => {
        impl<'a> Parsable<'a> for $output {
            type Parser = $parser;
        }
    };
}

pub trait Parsable<'a>: Sized {
    type Parser: Parser<'a, Output = Self>;
}
pub trait Parser<'a>: Sized {
    type Output;
    fn parse(_: &'a str) -> Option<Result<(Self::Output, &'a str), ParserError<'a>>>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParserError<'a> {
    UnusedEscape { src: &'a str, index: usize },
    UnknownSpecialKey(&'a str),
    UnclosedSpecialKey(&'a str),
}

pub struct KeyParser;
impl<'a> Parser<'a> for KeyParser {
    type Output = Key<'a>;

    fn parse(input: &'a str) -> Option<Result<(Key<'a>, &'a str), ParserError<'a>>> {
        let (modifiers, input) = match KeyModifiersParser::parse(input).transpose() {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        }
        .unwrap_or_else(|| (KeyModifiers::default(), input));
        let (kind, input) = match KeyKindParser::parse(input)? {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        };

        Some(Ok((Key::new(modifiers, kind), input)))
    }
}
impl_parsable_for!(Key<'a>, KeyParser);

pub struct KeyKindParser;
impl<'a> Parser<'a> for KeyKindParser {
    type Output = KeyKind<'a>;

    fn parse(input: &'a str) -> Option<Result<(Self::Output, &'a str), ParserError<'a>>> {
        match input {
            "" => None,
            input if input.starts_with('<') => InvisibleKeyParser::parse(input)
                .map(|result| result.map(|(key, next)| (KeyKind::Invisible(key), next))),
            input => {
                let mut keys = Cow::Borrowed("");
                let mut chars = input.char_indices().peekable();

                while let Some((index, ch)) =
                    chars.next_if(|(_, ch)| !matches!(ch, 'M' | 'C' | 'S' | 'L' | '<'))
                {
                    match ch {
                        '\\' => keys.to_mut().push(
                            match chars
                                .next()
                                .ok_or(ParserError::UnusedEscape { src: input, index })
                            {
                                Ok((_, ch)) => match ch {
                                    'n' => '\n',
                                    'r' => '\r',
                                    't' => '\t',
                                    ch => ch,
                                },
                                Err(err) => return Some(Err(err)),
                            },
                        ),
                        ch => match &mut keys {
                            Cow::Borrowed(keys) => *keys = &input[..=index],
                            Cow::Owned(keys) => keys.push(ch),
                        },
                    }
                }
                if keys.is_empty() {
                    return None;
                }

                Some(Ok((
                    KeyKind::Visible(keys),
                    chars
                        .next()
                        .and_then(|(i, _)| input.get(i..))
                        .unwrap_or(&input[input.len()..]),
                )))
            }
        }
    }
}
impl_parsable_for!(KeyKind<'a>, KeyKindParser);

pub struct InvisibleKeyParser;
impl<'a> Parser<'a> for InvisibleKeyParser {
    type Output = InvisibleKey;

    fn parse(input: &'a str) -> Option<Result<(Self::Output, &'a str), ParserError<'a>>> {
        if input.is_empty() || !input.starts_with('<') {
            None
        } else if let Some(end) = input.find('>') {
            let next = &input[end..];

            match &input[1..end] {
                "PG-UP" => Some(Ok((InvisibleKey::PageUp, next))),
                "PG-DN" => Some(Ok((InvisibleKey::PageDown, next))),
                fkey if fkey.starts_with("F-") => fkey
                    .chars()
                    .skip(2)
                    .try_fold(0_u8, |fold, next| {
                        let err = || ParserError::UnknownSpecialKey(fkey);

                        fold.checked_mul(10)
                            .ok_or_else(err)?
                            .checked_add(next.to_digit(10).ok_or_else(err)? as u8)
                            .ok_or_else(err)
                    })
                    .map(|i| (InvisibleKey::F(i), next))
                    .map(Some)
                    .transpose(),
                unknown => Some(Err(ParserError::UnknownSpecialKey(unknown))),
            }
        } else {
            Some(Err(ParserError::UnclosedSpecialKey(input)))
        }
    }
}
impl_parsable_for!(InvisibleKey, InvisibleKeyParser);

pub struct KeyModifierParser;
impl<'a> Parser<'a> for KeyModifierParser {
    type Output = KeyModifier;

    fn parse(input: &'a str) -> Option<Result<(KeyModifier, &'a str), ParserError<'a>>> {
        match input {
            input if input.starts_with("M-") => Some(Ok((KeyModifier::Alt, &input[2..]))),
            input if input.starts_with("C-") => Some(Ok((KeyModifier::Control, &input[2..]))),
            input if input.starts_with("S-") => Some(Ok((KeyModifier::Shift, &input[2..]))),
            input if input.starts_with("L-") => Some(Ok((KeyModifier::Super, &input[2..]))),
            _ => None,
        }
    }
}
impl_parsable_for!(KeyModifier, KeyModifierParser);

pub struct KeyModifiersParser;
impl<'a> Parser<'a> for KeyModifiersParser {
    type Output = KeyModifiers;

    fn parse(mut input: &'a str) -> Option<Result<(KeyModifiers, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_mods = KeyModifiers::default();

        while let Some((key_mod, next_input)) = KeyModifierParser::parse(input).transpose().expect("internal error: the implementation of [KeyModifierParser::parse] should never return any errors")
        {
            some = true;
            key_mods.push(key_mod);

            input = next_input;
        }

        some.then_some(Ok((key_mods, input)))
    }
}
impl_parsable_for!(KeyModifiers, KeyModifiersParser);

pub struct KeySequenceParser;
impl<'a> Parser<'a> for KeySequenceParser {
    type Output = KeySequence<'a>;
    fn parse(mut input: &'a str) -> Option<Result<(KeySequence<'a>, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_seq = KeySequence::new();

        while let Some((key, next_input)) = match KeyParser::parse(input).transpose() {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        } {
            some = true;
            key_seq.push(key);
            input = next_input;
        }

        some.then_some(Ok((key_seq, input)))
    }
}
impl_parsable_for!(KeySequence<'a>, KeySequenceParser);

#[cfg(test)]
mod tests {
    use {
        super::*,
        itertools::Itertools,
        std::fmt::{Debug, Display},
    };

    fn test_empty_parser<T>()
    where
        T: for<'a> Parser<'a>,
        for<'a> <T as Parser<'a>>::Output: Debug + PartialEq,
    {
        assert_eq!(T::parse(""), None);
    }

    fn test_parser<I, S, T>(iter: I)
    where
        I: IntoIterator<Item = (S, T)>,
        S: AsRef<str>,
        T: Debug + Display + for<'a> Parsable<'a> + PartialEq,
    {
        iter.into_iter().for_each(|(input, output)| {
            let parser_output = <T as Parsable>::Parser::parse(input.as_ref())
                .unwrap()
                .unwrap()
                .0;
            assert_eq!(parser_output, output);

            let display_output = format!("{}", parser_output);
            let display_output = <T as Parsable>::Parser::parse(&display_output)
                .unwrap()
                .unwrap()
                .0;
            assert_eq!(display_output, output);
        })
    }

    #[test]
    fn invisible_key() {
        test_empty_parser::<InvisibleKeyParser>();
        test_parser((u8::MIN..=u8::MAX).map(|i| (format!("<F-{}>", i), InvisibleKey::F(i))));
        test_parser([
            ("<PG-UP>", InvisibleKey::PageUp),
            ("<PG-DN>", InvisibleKey::PageDown),
        ]);
    }

    #[test]
    fn key_modifier() {
        test_empty_parser::<KeyModifierParser>();
        test_parser(KeyModifier::VARIANTS);
    }

    #[test]
    fn key_modifiers() {
        test_empty_parser::<KeyModifiersParser>();
        test_parser(
            (1..=4)
                .flat_map(|n| KeyModifier::VARIANTS.into_iter().permutations(n))
                .map(|modifiers| modifiers.into_iter().unzip::<_, _, String, KeyModifiers>()),
        );
    }

    #[test]
    fn key_kind() {
        test_empty_parser::<KeyKindParser>();
        assert_eq!(
            KeyKindParser::parse("foo bar<F-10>"),
            Some(Ok((KeyKind::Visible("foo bar".into()), "<F-10>"))),
        );

        const EXPECTED: KeyKind = KeyKind::Visible(Cow::Borrowed("<C"));

        let escaped_key = KeyKindParser::parse("\\<\\C").unwrap().unwrap().0;
        assert_eq!(escaped_key, EXPECTED);

        let display_key = format!("{}", escaped_key);
        let display_key = KeyKindParser::parse(&display_key).unwrap().unwrap().0;
        assert_eq!(display_key, EXPECTED);
    }

    // `Key*Parser` types are just the above combined, so they probably won't have any serializing/deserializing problems.
    // Their lifetimes do not allow them to use [test_parser]
    #[test]
    fn key() {
        test_empty_parser::<KeyParser>();
        assert_eq!(
            KeyParser::parse("foo"),
            Some(Ok((Key::new(Default::default(), "foo".into()), "")))
        );
    }
}
