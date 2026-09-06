// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fixed-capacity ring buffer for telemetry samples.

/// Overwrite-oldest ring buffer. Iteration yields elements oldest → newest.
pub struct Ring<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> Ring<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self {
            buf: vec![None; capacity],
            head: 0,
            len: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, value: T) {
        self.buf[self.head] = Some(value);
        self.head = (self.head + 1) % self.buf.len();
        self.len = (self.len + 1).min(self.buf.len());
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let cap = self.buf.len();
        let start = (self.head + cap - self.len) % cap;
        (0..self.len).filter_map(move |i| self.buf[(start + i) % cap].as_ref())
    }

    pub fn latest(&self) -> Option<&T> {
        if self.len == 0 {
            return None;
        }
        let cap = self.buf.len();
        self.buf[(self.head + cap - 1) % cap].as_ref()
    }

    /// Up to the last `n` elements, oldest → newest.
    pub fn last_n(&self, n: usize) -> Vec<T> {
        let take = n.min(self.len);
        self.iter().skip(self.len - take).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_wrap() {
        let mut r = Ring::new(3);
        assert!(r.is_empty());
        r.push(1);
        r.push(2);
        r.push(3);
        assert_eq!(r.iter().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        r.push(4); // overwrites 1
        assert_eq!(r.len(), 3);
        assert_eq!(r.iter().copied().collect::<Vec<_>>(), vec![2, 3, 4]);
        assert_eq!(r.latest(), Some(&4));
    }

    #[test]
    fn last_n_clamps() {
        let mut r = Ring::new(5);
        for i in 0..3 {
            r.push(i);
        }
        assert_eq!(r.last_n(2), vec![1, 2]);
        assert_eq!(r.last_n(10), vec![0, 1, 2]);
    }

    #[test]
    fn empty_ring() {
        let r: Ring<u32> = Ring::new(4);
        assert_eq!(r.latest(), None);
        assert!(r.last_n(3).is_empty());
        assert_eq!(r.iter().count(), 0);
    }
}
