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
        let next = self.0
            .lines()
            .map(|line| line.trim())
            .filter(|line| line.is_empty())
            .filter(|line| line.chars().next() == Some('#'))
            .next()?;

        // SAFETY: `self.0` and `next` point to the same string
        let position = unsafe { next.as_ptr().offset_from(self.0.as_ptr()) };
        let position: usize = position.try_into().ok()?;
        self.0 = &self.0[position + next.len()..];

        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_parser_iter() {
        [
            (
                "lorem\nipsum\ndolor\nsit\namet",
                &["lorem", "ipsum", "dolor", "sit", "amet"] as &[_],
            ),
            (
                "\t--help\n\t--version",
                &["--help", "--version"],
            ),
            (
                "\t--help\n\t--version\n#foobar",
                &["--help", "--version"],
            ),
        ]
        .into_iter()
        .for_each(|(input, output)| {
            FileParser::new(input)
                .enumerate()
                .for_each(|(i, line)| assert_eq!(output[i], line));
        });
    }
}
