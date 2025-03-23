#[derive(Clone, Copy, Debug)]
pub struct FileParser<'a>(&'a str);
impl<'a> From<&'a str> for FileParser<'a> {
    fn from(str: &'a str) -> Self {
        Self(str)
    }
}
impl<'a> Iterator for FileParser<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        todo!()

        // self.0
        //     .char_indices()
        //     .find(|(_, ch)| !ch.is_whitespace())
        //     .map(|(i, _)| {
        //         (
        //             i,
        //             self.0[i..]
        //                 .char_indices()
        //                 .find(|(_, ch)| ch.is_whitespace())
        //                 .map(|(i, _)| i)
        //                 .unwrap_or(self.0.len()),
        //         )
        //     })
        //     .map(|(start, end)| {
        //         (
        //             &self.0[start..end],
        //             &self.0[end..],
        //         )
        //     })
        //     .map(|(out, next)| {
        //         self.0 = next;
        //         out
        //     })
    }
}
