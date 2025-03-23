pub enum Recursion<T, E> {
    End(E),
    Continue(T),
}
impl<T, E> Recursion<T, E> {
    #[inline]
    pub fn start<F>(with: T, mut operation: F) -> E
    where F: FnMut(T) -> Self {
        let mut last = with;

        loop {
            match operation(last) {
                Self::End(end) => return end,
                Self::Continue(with) => last = with,
            }
        }
    }
}
