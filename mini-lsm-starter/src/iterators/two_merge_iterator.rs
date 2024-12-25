#![allow(unused_variables)] // TODO(you): remove this lint after implementing this mod
#![allow(dead_code)] // TODO(you): remove this lint after implementing this mod

use anyhow::Result;

use super::StorageIterator;

/// Merges two iterators of different types into one. If the two iterators have the same key, only
/// produce the key once and prefer the entry from A.
pub struct TwoMergeIterator<A: StorageIterator, B: StorageIterator> {
    a: A,
    b: B,
    read_from_a: bool,
}

impl<
        A: 'static + StorageIterator,
        B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
    > TwoMergeIterator<A, B>
{
    pub fn create(a: A, b: B) -> Result<Self> {
        let read_from_a = if a.is_valid() && b.is_valid() && a.key() <= b.key() {
            true
        } else if a.is_valid() && !b.is_valid() {
            true
        } else {
            false
        };

        let mut iterator = TwoMergeIterator { a, b, read_from_a };
        while iterator.b.is_valid() && iterator.a.is_valid() && iterator.b.key() == iterator.a.key()
        {
            iterator.b.next()?;
        }
        Ok(iterator)
    }
}

impl<
        A: 'static + StorageIterator,
        B: 'static + for<'a> StorageIterator<KeyType<'a> = A::KeyType<'a>>,
    > StorageIterator for TwoMergeIterator<A, B>
{
    type KeyType<'a> = A::KeyType<'a>;

    fn value(&self) -> &[u8] {
        if self.read_from_a {
            self.a.value()
        } else {
            self.b.value()
        }
    }

    fn key(&self) -> Self::KeyType<'_> {
        if self.read_from_a {
            self.a.key()
        } else {
            self.b.key()
        }
    }

    fn is_valid(&self) -> bool {
        if self.read_from_a {
            self.a.is_valid()
        } else {
            self.b.is_valid()
        }
    }

    fn next(&mut self) -> Result<()> {
        if self.read_from_a {
            self.a.next()?;
            if self.a.is_valid() {
                while self.b.is_valid() && self.b.key() == self.a.key() {
                    self.b.next()?;
                }
                if self.b.is_valid() && self.a.key() > self.b.key() {
                    self.read_from_a = false;
                }
            } else {
                self.read_from_a = false;
            }
        } else {
            self.b.next()?;
            if self.b.is_valid() {
                while self.b.is_valid() && self.a.is_valid() && self.b.key() == self.a.key() {
                    self.b.next()?;
                }
                if self.b.is_valid() && self.a.is_valid() && self.a.key() < self.b.key()
                    || !self.b.is_valid()
                {
                    self.read_from_a = true;
                }
            } else {
                self.read_from_a = true;
            }
        }

        Ok(())
    }
}
