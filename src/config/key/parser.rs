use {
    super::{InvisibleKey, Key, KeyKind, KeyModifier, KeyModifiers, KeySequence},
    std::{borrow::Cow, str},
};

pub trait Parser<'a>: Sized {
    fn parse(_: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParserError<'a> {
    UnusedEscape { src: &'a str, index: usize },
    UnknownSpecialKey(&'a str),
    UnclosedSpecialKey(&'a str),
}

impl<'a> Parser<'a> for Key<'a> {
    fn parse(input: &'a str) -> Option<Result<(Key<'a>, &'a str), ParserError<'a>>> {
        let (modifiers, input) = match KeyModifiers::parse(input).transpose() {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        }
        .unwrap_or_else(|| (KeyModifiers::default(), input));
        let (kind, input) = match KeyKind::parse(input)? {
            Ok(o) => o,
            Err(err) => return Some(Err(err)),
        };

        Some(Ok((Key::new(modifiers, kind), input)))
    }
}

impl<'a> Parser<'a> for KeyKind<'a> {
    fn parse(input: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>> {
        match input {
            "" => None,
            input if input.starts_with('<') => InvisibleKey::parse(input)
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

impl<'a> Parser<'a> for InvisibleKey {
    fn parse(input: &'a str) -> Option<Result<(Self, &'a str), ParserError<'a>>> {
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

impl<'a> Parser<'a> for KeyModifier {
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

impl<'a> Parser<'a> for KeyModifiers {
    fn parse(mut input: &'a str) -> Option<Result<(KeyModifiers, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_mods = KeyModifiers::default();

        while let Some((key_mod, next_input)) = KeyModifier::parse(input).transpose().expect("internal error: the implementation of [KeyModifierParser::parse] should never return any errors")
        {
            some = true;
            key_mods.push(key_mod);

            input = next_input;
        }

        some.then_some(Ok((key_mods, input)))
    }
}

impl<'a> Parser<'a> for KeySequence<'a> {
    fn parse(mut input: &'a str) -> Option<Result<(KeySequence<'a>, &'a str), ParserError<'a>>> {
        let mut some = false;
        let mut key_seq = KeySequence::new();

        while let Some((key, next_input)) = match Key::parse(input).transpose() {
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        itertools::Itertools,
        std::fmt::{Debug, Display, Write},
    };

    /// [test_parser_eq] with format checking.
    fn test_parser<I, S, T>(iter: I)
    where
        I: IntoIterator<Item = (S, T)>,
        S: AsRef<str>,
        T: Debug + Display + for<'a> Parser<'a> + PartialEq,
    {
        let mut input_buffer = String::new();
        let mut expected_buffer = String::new();

        iter.into_iter().for_each(|(input, expected)| {
            let input = T::parse(input.as_ref()).unwrap().unwrap().0;
            assert_eq!(input, expected);

            [
                (&mut input_buffer, &input),
                (&mut expected_buffer, &expected),
            ]
            .into_iter()
            .for_each(|(buffer, token)| {
                buffer.clear();
                write!(buffer, "{}", token).unwrap();
            });
            assert_eq!(input_buffer, expected_buffer);

            let display = T::parse(&expected_buffer).unwrap().unwrap().0;
            assert_eq!(display, expected);
        })
    }

    #[test]
    fn invisible_key() {
        test_parser((u8::MIN..=u8::MAX).map(|i| (format!("<F-{}>", i), InvisibleKey::F(i))));
        test_parser([
            ("<PG-UP>", InvisibleKey::PageUp),
            ("<PG-DN>", InvisibleKey::PageDown),
        ]);
    }

    #[test]
    fn key_modifier() {
        test_parser(KeyModifier::VARIANTS);
    }

    #[test]
    fn key_modifiers() {
        test_parser(
            (1..=4)
                .flat_map(|n| KeyModifier::VARIANTS.into_iter().permutations(n))
                .map(|modifiers| modifiers.into_iter().unzip::<_, _, String, KeyModifiers>()),
        );
    }

    #[test]
    fn key_kind() {
        assert_eq!(
            KeyKind::parse("foo bar<F-10>"),
            Some(Ok((KeyKind::Visible("foo bar".into()), "<F-10>"))),
        );

        const EXPECTED: KeyKind = KeyKind::Visible(Cow::Borrowed("<C"));

        let escaped_key = KeyKind::parse("\\<\\C").unwrap().unwrap().0;
        assert_eq!(escaped_key, EXPECTED);

        let display_key = format!("{}", escaped_key);
        let display_key = KeyKind::parse(&display_key).unwrap().unwrap().0;
        assert_eq!(display_key, EXPECTED);
    }

    // `Key*` types are just the above combined, so they won't have anything worth testing.
}
