//! A list that is never changed in place and shares the parts it has in common with another.
//!
//! The items are cut into chunks where the content says (an item whose hash ends in the right
//! bits, after at least `MIN` items; at most `MAX` in one), so that the same run of items is cut
//! the same way wherever it is in the list. [`ChunkedVec::sharing`] takes the chunks of an older
//! list that the new one has unchanged, so a take made by moving one note keeps one or two new
//! chunks and shares the rest with the take it was made from (docs/architecture.md 16).

use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Index;
use std::sync::Arc;

const MIN: usize = 64;
/// After `MIN` items, an item ends a chunk 1 time in `SPREAD`: 192 items a chunk on average.
const SPREAD: u64 = 128;
const MAX: usize = 512;

#[derive(Clone)]
struct Chunk<T> {
    /// The index of the first item in the whole list.
    at: usize,
    hash: u64,
    items: Arc<[T]>,
}

#[derive(Clone)]
pub struct ChunkedVec<T> {
    /// The first chunk, kept here so that a short list is one step away.
    head: Option<Chunk<T>>,
    rest: Box<[Chunk<T>]>,
    len: usize,
}

impl<T> Default for ChunkedVec<T> {
    fn default() -> Self {
        ChunkedVec {
            head: None,
            rest: Box::new([]),
            len: 0,
        }
    }
}

impl<T: Clone + Eq + Hash> ChunkedVec<T> {
    /// `items` in chunks; each chunk equal to one of `old` is that chunk of `old`, shared.
    pub fn sharing(items: &[T], old: &ChunkedVec<T>) -> Self {
        let mut by_hash: Vec<(u64, &Arc<[T]>)> = old.chunks().map(|c| (c.hash, &c.items)).collect();
        by_hash.sort_unstable_by_key(|&(k, _)| k);
        let mut chunks = Vec::new();
        let mut at = 0;
        let mut hash = START;
        for (i, item) in items.iter().enumerate() {
            let h = item_hash(item);
            hash = combine(hash, h);
            let n = i + 1 - at;
            let last = i + 1 == items.len();
            if last || n >= MAX || (n >= MIN && mix(h).is_multiple_of(SPREAD)) {
                let piece = &items[at..=i];
                let found = by_hash[by_hash.partition_point(|&(k, _)| k < hash)..]
                    .iter()
                    .take_while(|&&(k, _)| k == hash)
                    .map(|&(_, c)| c)
                    .find(|c| c[..] == *piece);
                chunks.push(Chunk {
                    at,
                    hash,
                    items: found.cloned().unwrap_or_else(|| piece.into()),
                });
                at = i + 1;
                hash = START;
            }
        }
        let mut chunks = chunks.into_iter();
        ChunkedVec {
            head: chunks.next(),
            rest: chunks.collect(),
            len: items.len(),
        }
    }
}

impl<T> ChunkedVec<T> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    fn chunks(&self) -> impl DoubleEndedIterator<Item = &Chunk<T>> {
        self.head.iter().chain(self.rest.iter())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        let head = self.head.as_ref()?;
        if let Some(item) = head.items.get(index) {
            return Some(item);
        }
        let i = self
            .rest
            .partition_point(|c| c.at <= index)
            .checked_sub(1)?;
        let c = &self.rest[i];
        c.items.get(index - c.at)
    }

    /// Like [`slice::partition_point`]: the index of the first item for which `pred` is false,
    /// when it is true for all items before that one and false for all after.
    pub fn partition_point(&self, mut pred: impl FnMut(&T) -> bool) -> usize {
        let Some(head) = &self.head else {
            return 0;
        };
        if !head.items.last().is_some_and(&mut pred) {
            return head.items.partition_point(pred);
        }
        let i = self
            .rest
            .partition_point(|c| c.items.last().is_some_and(&mut pred));
        match self.rest.get(i) {
            Some(c) => c.at + c.items.partition_point(pred),
            None => self.len,
        }
    }

    pub fn first(&self) -> Option<&T> {
        self.chunks().next().and_then(|c| c.items.first())
    }

    pub fn last(&self) -> Option<&T> {
        self.chunks().next_back().and_then(|c| c.items.last())
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            chunks: self.rest.iter(),
            front: self.head.as_ref().map_or(&[][..], |c| &c.items).iter(),
            back: [].iter(),
        }
    }

    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.iter().cloned().collect()
    }

    /// The bytes this list holds that `other` does not share with it: its chunks and their
    /// list, counted as allocated (Arc counts included).
    pub fn bytes_not_in(&self, other: &ChunkedVec<T>) -> usize {
        let arc_head = 2 * size_of::<usize>();
        size_of_val::<[Chunk<T>]>(&self.rest)
            + self
                .chunks()
                .filter(|c| !other.chunks().any(|o| Arc::ptr_eq(&o.items, &c.items)))
                .map(|c| arc_head + size_of_val::<[T]>(&c.items))
                .sum::<usize>()
    }

    /// The length of each chunk, in order.
    pub fn chunk_lens(&self) -> impl Iterator<Item = usize> + '_ {
        self.chunks().map(|c| c.items.len())
    }
}

impl<T: Clone + Eq + Hash> From<Vec<T>> for ChunkedVec<T> {
    fn from(items: Vec<T>) -> Self {
        ChunkedVec::sharing(&items, &ChunkedVec::default())
    }
}

impl<T: Clone + Eq + Hash> FromIterator<T> for ChunkedVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        iter.into_iter().collect::<Vec<T>>().into()
    }
}

impl<T> Index<usize> for ChunkedVec<T> {
    type Output = T;

    fn index(&self, index: usize) -> &T {
        match self.get(index) {
            Some(item) => item,
            None => panic!("index {index} out of {}", self.len),
        }
    }
}

impl<T: PartialEq> PartialEq for ChunkedVec<T> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl<T: Eq> Eq for ChunkedVec<T> {}

impl<T: PartialEq> PartialEq<[T]> for ChunkedVec<T> {
    fn eq(&self, other: &[T]) -> bool {
        self.len == other.len() && self.iter().eq(other.iter())
    }
}

impl<T: PartialEq> PartialEq<Vec<T>> for ChunkedVec<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        *self == **other
    }
}

impl<T: PartialEq, const N: usize> PartialEq<[T; N]> for ChunkedVec<T> {
    fn eq(&self, other: &[T; N]) -> bool {
        *self == other[..]
    }
}

impl<T: fmt::Debug> fmt::Debug for ChunkedVec<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

impl<'a, T> IntoIterator for &'a ChunkedVec<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[derive(Clone)]
pub struct Iter<'a, T> {
    chunks: std::slice::Iter<'a, Chunk<T>>,
    front: std::slice::Iter<'a, T>,
    back: std::slice::Iter<'a, T>,
}

impl<'a, T> Iter<'a, T> {
    #[inline(never)]
    fn next_chunk(&mut self) -> Option<&'a T> {
        loop {
            match self.chunks.next() {
                Some(c) => self.front = c.items.iter(),
                None => return self.back.next(),
            }
            if let Some(item) = self.front.next() {
                return Some(item);
            }
        }
    }
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        match self.front.next() {
            Some(item) => Some(item),
            None => self.next_chunk(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let middle = match self.chunks.as_slice() {
            [] => 0,
            [first, .., last] => last.at + last.items.len() - first.at,
            [only] => only.items.len(),
        };
        let left = self.front.len() + middle + self.back.len();
        (left, Some(left))
    }

    fn fold<B, F: FnMut(B, &'a T) -> B>(self, init: B, mut f: F) -> B {
        let mut acc = self.front.fold(init, &mut f);
        for c in self.chunks {
            acc = c.items.iter().fold(acc, &mut f);
        }
        self.back.fold(acc, f)
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(item) = self.back.next_back() {
                return Some(item);
            }
            match self.chunks.next_back() {
                Some(c) => self.back = c.items.iter(),
                None => return self.front.next_back(),
            }
        }
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

const START: u64 = 0xcbf2_9ce4_8422_2325;
const K: u64 = 0x517c_c1b7_2722_0a95;

fn combine(a: u64, b: u64) -> u64 {
    (a.rotate_left(5) ^ b).wrapping_mul(K)
}

/// Spreads every bit of `h` over the low ones (splitmix64's last step).
fn mix(mut h: u64) -> u64 {
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

fn item_hash<T: Hash>(item: &T) -> u64 {
    let mut h = Quick(START);
    item.hash(&mut h);
    h.0
}

/// A fast hash, not for anything that must resist chosen input.
struct Quick(u64);

impl Hasher for Quick {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes.chunks(8) {
            let mut word = [0; 8];
            word[..b.len()].copy_from_slice(b);
            self.write_u64(u64::from_le_bytes(word));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.write_u64(u64::from(i));
    }

    fn write_u16(&mut self, i: u16) {
        self.write_u64(u64::from(i));
    }

    fn write_u32(&mut self, i: u32) {
        self.write_u64(u64::from(i));
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = combine(self.0, i);
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    fn write_isize(&mut self, i: isize) {
        self.write_u64(i as u64);
    }
}

#[cfg(test)]
mod tests;
