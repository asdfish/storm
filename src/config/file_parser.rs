use crate::recursion::Recursion;

#[derive(Clone, Copy, Debug)]
pub struct FileParser<'a>(&'a str);
impl<'a> FileParser<'a> {
    pub const fn new(input: &'a str) -> Self {
        Self(input)
    }
}
impl<'a> From<&'a str> for FileParser<'a> {
    fn from(input: &'a str) -> Self {
        Self(input)
    }
}
impl<'a> Iterator for FileParser<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        Recursion::start(self, |s| {
            let mut chars = s.0.char_indices();

            let (start, _) = match chars.by_ref()
                .skip_while(|(_, ch)| ch.is_whitespace())
                .next() {
                    Some((_, '#')) => {
                        let _ = chars.by_ref()
                            .skip_while(|(_, ch)| !ch.is_whitespace())
                            .next();
                        s.0 = chars.as_str();
                        return Recursion::Continue(s);
                    },
                    Some(start) => start,
                    None => return Recursion::End(None),
            };
            let end = chars.by_ref()
                .find(|(_, ch)| ch.is_whitespace())
                .map(|(i, _)| i)
                .unwrap_or(s.0.len());

            let line = &s.0[start..end];
            s.0 = &s.0[end..];

            Recursion::End(Some(line))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_parser_iter() {
        [
            ("lorem\nipsum\ndolor\nsit\namet", &[
                "lorem",
                "ipsum",
                "dolor",
                "sit",
                "amet",
            ] as &[_])
        ]
            .into_iter()
            .for_each(|(input, output)| {
                FileParser::new(input)
                    .enumerate()
                    .for_each(|(i, line)| assert_eq!(output[i], line));
            });
    }
}
