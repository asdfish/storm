use std::{
    iter::FusedIterator,
    marker::PhantomData,
};

pub trait IterExt: Iterator {
    /// [Iterator::zip] until both iterators have run out
    fn zip_all<I, R, T>(self, r: I) -> ZipAll<Self, R, Self::Item, T>
    where I: IntoIterator<Item = T, IntoIter = R>,
        R: Iterator<Item = T>,
        Self: Sized {
        ZipAll {
            l: self,
            r: r.into_iter(),
            _marker: PhantomData,
        }
    }
}
impl<T> IterExt for T where T: Iterator {}

pub struct ZipAll<L, R, LT, RT>
where L: Iterator<Item = LT>,
R: Iterator<Item = RT> {
    l: L,
    r: R,
    _marker: PhantomData<(LT, RT)>,
}
impl<L, R, LT, RT> Iterator for ZipAll<L, R, LT, RT>
where L: Iterator<Item = LT>,
R: Iterator<Item = RT> {
    type Item = (Option<LT>, Option<RT>);

    fn next(&mut self) -> Option<(Option<LT>, Option<RT>)> {
        match (self.l.next(), self.r.next()) {
            (None, None) => None,
            zip => Some(zip),
        }
    }
}
impl<L, R, LT, RT> FusedIterator for ZipAll<L, R, LT, RT>
where L: Iterator<Item = LT> + FusedIterator,
R: Iterator<Item = RT> + FusedIterator {}

#[cfg(test)]
mod tests {
    use super::IterExt;

    #[test]
    fn zip_all() {
        let mut iter = ["foo", "bar"].into_iter().zip_all(["baz"]);
        assert_eq!(iter.next(), Some((Some("foo"), Some("baz"))));
        assert_eq!(iter.next(), Some((Some("bar"), None)));
        assert_eq!(iter.next(), None);
    }
}
