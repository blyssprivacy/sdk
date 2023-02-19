/// A simple wrapper around an iterator that overrides the inner
/// iterator at exactly one point.
///
/// ```
/// # use doublepir_rs::util::FixedPointIter;
/// let iter1 = vec![1,2,3].into_iter();
/// let mut iter2 = FixedPointIter::new(iter1, 1, 99);
/// assert_eq!(iter2.next().unwrap(), 1);
/// assert_eq!(iter2.next().unwrap(), 99);
/// assert_eq!(iter2.next().unwrap(), 3);
/// ```
pub struct FixedPointIter<I: Iterator>
where
    I::Item: Clone,
{
    iter: I,
    point: usize,
    value: I::Item,
    idx: usize,
}

impl<I: Iterator> FixedPointIter<I>
where
    I::Item: Clone,
{
    pub fn new(iter: I, point: usize, value: I::Item) -> Self {
        Self {
            iter,
            point,
            value,
            idx: 0,
        }
    }
}

impl<I: Iterator> Iterator for FixedPointIter<I>
where
    I::Item: Clone,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let mut next_val = self.iter.next();
        if self.idx == self.point {
            next_val = Some(self.value.clone());
        }
        self.idx += 1;
        return next_val;
    }
}
