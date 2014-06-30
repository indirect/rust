// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(missing_doc)]

use core::prelude::*;

use core::cmp;
use core::default::Default;
use core::fmt;
use core::iter::{Map, Zip};
use core::ops;
use core::slice;
use core::uint;
use std::hash;

use {Collection, Mutable, Set, MutableSet};
use vec::Vec;

/// The bitvector type
///
/// # Example
///
/// ```rust
/// use collections::bitv::Bitv;
///
/// let mut bv = Bitv::new(10, false);
///
/// // insert all primes less than 10
/// bv.set(2, true);
/// bv.set(3, true);
/// bv.set(5, true);
/// bv.set(7, true);
/// println!("{}", bv.to_str());
/// println!("total bits set to true: {}", bv.iter().filter(|x| *x).count());
///
/// // flip all values in bitvector, producing non-primes less than 10
/// bv.negate();
/// println!("{}", bv.to_str());
/// println!("total bits set to true: {}", bv.iter().filter(|x| *x).count());
///
/// // reset bitvector to empty
/// bv.clear();
/// println!("{}", bv.to_str());
/// println!("total bits set to true: {}", bv.iter().filter(|x| *x).count());
/// ```
#[deriving(Clone)]
pub struct Bitv {
    /// Internal representation of the bit vector
    storage: Vec<uint>,
    /// The number of valid bits in the internal representation
    nbits: uint
}

struct MaskWords<'a> {
    iter: slice::Items<'a, uint>,
    next_word: Option<&'a uint>,
    last_word_mask: uint,
    offset: uint
}

impl<'a> Iterator<(uint, uint)> for MaskWords<'a> {
    /// Returns (offset, word)
    fn next<'a>(&'a mut self) -> Option<(uint, uint)> {
        let ret = self.next_word;
        match ret {
            Some(&w) => {
                self.next_word = self.iter.next();
                self.offset += 1;
                // The last word may need to be masked
                if self.next_word.is_none() {
                    Some((self.offset - 1, w & self.last_word_mask))
                } else {
                    Some((self.offset - 1, w))
                }
            },
            None => None
        }
    }
}

impl Bitv {
    #[inline]
    fn process(&mut self, other: &Bitv, op: |uint, uint| -> uint) -> bool {
        let len = other.storage.len();
        assert_eq!(self.storage.len(), len);
        let mut changed = false;
        // Notice: `a` is *not* masked here, which is fine as long as
        // `op` is a bitwise operation, since any bits that should've
        // been masked were fine to change anyway. `b` is masked to
        // make sure its unmasked bits do not cause damage.
        for (a, (_, b)) in self.storage.mut_iter()
                           .zip(other.mask_words(0)) {
            let w = op(*a, b);
            if *a != w {
                changed = true;
                *a = w;
            }
        }
        changed
    }

    #[inline]
    fn mask_words<'a>(&'a self, mut start: uint) -> MaskWords<'a> {
        if start > self.storage.len() {
            start = self.storage.len();
        }
        let mut iter = self.storage.slice_from(start).iter();
        MaskWords {
          next_word: iter.next(),
          iter: iter,
          last_word_mask: {
              let rem = self.nbits % uint::BITS;
              if rem > 0 {
                  (1 << rem) - 1
              } else { !0 }
          },
          offset: start
        }
    }

    /// Creates an empty Bitv that holds `nbits` elements, setting each element
    /// to `init`.
    pub fn new(nbits: uint, init: bool) -> Bitv {
        Bitv {
            storage: Vec::from_elem((nbits + uint::BITS - 1) / uint::BITS,
                                    if init { !0u } else { 0u }),
            nbits: nbits
        }
    }

    /**
     * Calculates the union of two bitvectors
     *
     * Sets `self` to the union of `self` and `v1`. Both bitvectors must be
     * the same length. Returns `true` if `self` changed.
    */
    #[inline]
    pub fn union(&mut self, other: &Bitv) -> bool {
        self.process(other, |w1, w2| w1 | w2)
    }

    /**
     * Calculates the intersection of two bitvectors
     *
     * Sets `self` to the intersection of `self` and `v1`. Both bitvectors
     * must be the same length. Returns `true` if `self` changed.
    */
    #[inline]
    pub fn intersect(&mut self, other: &Bitv) -> bool {
        self.process(other, |w1, w2| w1 & w2)
    }

    /**
     * Assigns the value of `v1` to `self`
     *
     * Both bitvectors must be the same length. Returns `true` if `self` was
     * changed
     */
    #[inline]
    pub fn assign(&mut self, other: &Bitv) -> bool {
        self.process(other, |_, w| w)
    }

    /// Retrieve the value at index `i`
    #[inline]
    pub fn get(&self, i: uint) -> bool {
        assert!(i < self.nbits);
        let w = i / uint::BITS;
        let b = i % uint::BITS;
        let x = self.storage.get(w) & (1 << b);
        x != 0
    }

    /**
     * Set the value of a bit at a given index
     *
     * `i` must be less than the length of the bitvector.
     */
    #[inline]
    pub fn set(&mut self, i: uint, x: bool) {
        assert!(i < self.nbits);
        let w = i / uint::BITS;
        let b = i % uint::BITS;
        let flag = 1 << b;
        *self.storage.get_mut(w) = if x { *self.storage.get(w) | flag }
                          else { *self.storage.get(w) & !flag };
    }

    /// Set all bits to 0
    #[inline]
    pub fn clear(&mut self) {
        for w in self.storage.mut_iter() { *w = 0u; }
    }

    /// Set all bits to 1
    #[inline]
    pub fn set_all(&mut self) {
        for w in self.storage.mut_iter() { *w = !0u; }
    }

    /// Flip all bits
    #[inline]
    pub fn negate(&mut self) {
        for w in self.storage.mut_iter() { *w = !*w; }
    }

    /**
     * Calculate the difference between two bitvectors
     *
     * Sets each element of `v0` to the value of that element minus the
     * element of `v1` at the same index. Both bitvectors must be the same
     * length.
     *
     * Returns `true` if `v0` was changed.
     */
    #[inline]
    pub fn difference(&mut self, other: &Bitv) -> bool {
        self.process(other, |w1, w2| w1 & !w2)
    }

    /// Returns `true` if all bits are 1
    #[inline]
    pub fn all(&self) -> bool {
        let mut last_word = !0u;
        // Check that every word but the last is all-ones...
        self.mask_words(0).all(|(_, elem)|
            { let tmp = last_word; last_word = elem; tmp == !0u }) &&
        // ...and that the last word is ones as far as it needs to be
        (last_word == ((1 << self.nbits % uint::BITS) - 1) || last_word == !0u)
    }

    /// Returns an iterator over the elements of the vector in order.
    ///
    /// # Example
    ///
    /// ```rust
    /// use collections::bitv::Bitv;
    /// let mut bv = Bitv::new(10, false);
    /// bv.set(1, true);
    /// bv.set(2, true);
    /// bv.set(3, true);
    /// bv.set(5, true);
    /// bv.set(8, true);
    /// // Count bits set to 1; result should be 5
    /// println!("{}", bv.iter().filter(|x| *x).count());
    /// ```
    #[inline]
    pub fn iter<'a>(&'a self) -> Bits<'a> {
        Bits {bitv: self, next_idx: 0, end_idx: self.nbits}
    }

    /// Returns `true` if all bits are 0
    pub fn none(&self) -> bool {
        self.mask_words(0).all(|(_, w)| w == 0)
    }

    #[inline]
    /// Returns `true` if any bit is 1
    pub fn any(&self) -> bool {
        !self.none()
    }

    /**
     * Organise the bits into bytes, such that the first bit in the
     * `Bitv` becomes the high-order bit of the first byte. If the
     * size of the `Bitv` is not a multiple of 8 then trailing bits
     * will be filled-in with false/0
     */
    pub fn to_bytes(&self) -> Vec<u8> {
        fn bit (bitv: &Bitv, byte: uint, bit: uint) -> u8 {
            let offset = byte * 8 + bit;
            if offset >= bitv.nbits {
                0
            } else {
                bitv[offset] as u8 << (7 - bit)
            }
        }

        let len = self.nbits/8 +
                  if self.nbits % 8 == 0 { 0 } else { 1 };
        Vec::from_fn(len, |i|
            bit(self, i, 0) |
            bit(self, i, 1) |
            bit(self, i, 2) |
            bit(self, i, 3) |
            bit(self, i, 4) |
            bit(self, i, 5) |
            bit(self, i, 6) |
            bit(self, i, 7)
        )
    }

    /**
     * Compare a bitvector to a vector of `bool`.
     *
     * Both the bitvector and vector must have the same length.
     */
    pub fn eq_vec(&self, v: &[bool]) -> bool {
        assert_eq!(self.nbits, v.len());
        let mut i = 0;
        while i < self.nbits {
            if self.get(i) != v[i] { return false; }
            i = i + 1;
        }
        true
    }
}

/**
 * Transform a byte-vector into a `Bitv`. Each byte becomes 8 bits,
 * with the most significant bits of each byte coming first. Each
 * bit becomes `true` if equal to 1 or `false` if equal to 0.
 */
pub fn from_bytes(bytes: &[u8]) -> Bitv {
    from_fn(bytes.len() * 8, |i| {
        let b = bytes[i / 8] as uint;
        let offset = i % 8;
        b >> (7 - offset) & 1 == 1
    })
}

/**
 * Transform a `[bool]` into a `Bitv` by converting each `bool` into a bit.
 */
pub fn from_bools(bools: &[bool]) -> Bitv {
    from_fn(bools.len(), |i| bools[i])
}

/**
 * Create a `Bitv` of the specified length where the value at each
 * index is `f(index)`.
 */
pub fn from_fn(len: uint, f: |index: uint| -> bool) -> Bitv {
    let mut bitv = Bitv::new(len, false);
    for i in range(0u, len) {
        bitv.set(i, f(i));
    }
    bitv
}

impl ops::Index<uint,bool> for Bitv {
    fn index(&self, i: &uint) -> bool {
        self.get(*i)
    }
}

impl fmt::Show for Bitv {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for bit in self.iter() {
            try!(write!(fmt, "{}", if bit { 1u } else { 0u }));
        }
        Ok(())
    }
}

impl<S: hash::Writer> hash::Hash<S> for Bitv {
    fn hash(&self, state: &mut S) {
        self.nbits.hash(state);
        for (_, elem) in self.mask_words(0) {
            elem.hash(state);
        }
    }
}

impl cmp::PartialEq for Bitv {
    #[inline]
    fn eq(&self, other: &Bitv) -> bool {
        if self.nbits != other.nbits {
            return false;
        }
        self.mask_words(0).zip(other.mask_words(0)).all(|((_, w1), (_, w2))| w1 == w2)
    }
}

impl cmp::Eq for Bitv {}

#[inline]
fn iterate_bits(base: uint, bits: uint, f: |uint| -> bool) -> bool {
    if bits == 0 {
        return true;
    }
    for i in range(0u, uint::BITS) {
        if bits & (1 << i) != 0 {
            if !f(base + i) {
                return false;
            }
        }
    }
    return true;
}

/// An iterator for `Bitv`.
pub struct Bits<'a> {
    bitv: &'a Bitv,
    next_idx: uint,
    end_idx: uint,
}

impl<'a> Iterator<bool> for Bits<'a> {
    #[inline]
    fn next(&mut self) -> Option<bool> {
        if self.next_idx != self.end_idx {
            let idx = self.next_idx;
            self.next_idx += 1;
            Some(self.bitv.get(idx))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (uint, Option<uint>) {
        let rem = self.end_idx - self.next_idx;
        (rem, Some(rem))
    }
}

impl<'a> DoubleEndedIterator<bool> for Bits<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<bool> {
        if self.next_idx != self.end_idx {
            self.end_idx -= 1;
            Some(self.bitv.get(self.end_idx))
        } else {
            None
        }
    }
}

impl<'a> ExactSize<bool> for Bits<'a> {}

impl<'a> RandomAccessIterator<bool> for Bits<'a> {
    #[inline]
    fn indexable(&self) -> uint {
        self.end_idx - self.next_idx
    }

    #[inline]
    fn idx(&mut self, index: uint) -> Option<bool> {
        if index >= self.indexable() {
            None
        } else {
            Some(self.bitv.get(index))
        }
    }
}

/// An implementation of a set using a bit vector as an underlying
/// representation for holding numerical elements.
///
/// It should also be noted that the amount of storage necessary for holding a
/// set of objects is proportional to the maximum of the objects when viewed
/// as a `uint`.
#[deriving(Clone, PartialEq, Eq)]
pub struct BitvSet(Bitv);

impl Default for BitvSet {
    #[inline]
    fn default() -> BitvSet { BitvSet::new() }
}

impl BitvSet {
    /// Creates a new bit vector set with initially no contents
    pub fn new() -> BitvSet {
        BitvSet(Bitv::new(0, false))
    }

    /// Creates a new bit vector set from the given bit vector
    pub fn from_bitv(bitv: Bitv) -> BitvSet {
        BitvSet(bitv)
    }

    /// Returns the capacity in bits for this bit vector. Inserting any
    /// element less than this amount will not trigger a resizing.
    pub fn capacity(&self) -> uint {
        let &BitvSet(ref bitv) = self;
        bitv.storage.len() * uint::BITS
    }

    /// Consumes this set to return the underlying bit vector
    pub fn unwrap(self) -> Bitv {
        let BitvSet(bitv) = self;
        bitv
    }

    #[inline]
    /// Grows the vector to be able to store bits with indices `[0, size - 1]`
    fn grow(&mut self, size: uint) {
        let &BitvSet(ref mut bitv) = self;
        let old_size = bitv.storage.len();
        let size = (size + uint::BITS - 1) / uint::BITS;
        if old_size < size {
            bitv.storage.grow(size - old_size, &0);
        }
    }

    #[inline]
    fn other_op(&mut self, other: &BitvSet, f: |uint, uint| -> uint) {
        // Expand the vector if necessary
        self.grow(other.capacity());
        // Unwrap Bitvs
        let &BitvSet(ref mut self_bitv) = self;
        let &BitvSet(ref other_bitv) = other;
        for (i, w) in other_bitv.mask_words(0) {
            let old = *self_bitv.storage.get(i);
            let new = f(old, w);
            *self_bitv.storage.get_mut(i) = new;
        }
    }

    #[inline]
    /// Truncate the underlying vector to the least length required
    pub fn shrink_to_fit(&mut self) {
        let &BitvSet(ref mut bitv) = self;
        // Obtain original length
        let old_len = bitv.storage.len();
        // Obtain coarse trailing zero length
        let n = bitv.storage.iter().rev().take_while(|&&n| n == 0).count();
        // Truncate
        let trunc_len = cmp::max(old_len - n, 1);
        bitv.storage.truncate(trunc_len);
        bitv.nbits = trunc_len * uint::BITS;
    }

    /// Union in-place with the specified other bit vector
    pub fn union_with(&mut self, other: &BitvSet) {
        self.other_op(other, |w1, w2| w1 | w2);
    }

    /// Intersect in-place with the specified other bit vector
    pub fn intersect_with(&mut self, other: &BitvSet) {
        self.other_op(other, |w1, w2| w1 & w2);
    }

    /// Difference in-place with the specified other bit vector
    pub fn difference_with(&mut self, other: &BitvSet) {
        self.other_op(other, |w1, w2| w1 & !w2);
    }

    /// Symmetric difference in-place with the specified other bit vector
    pub fn symmetric_difference_with(&mut self, other: &BitvSet) {
        self.other_op(other, |w1, w2| w1 ^ w2);
    }

    pub fn iter<'a>(&'a self) -> BitPositions<'a> {
        BitPositions {set: self, next_idx: 0}
    }

    pub fn difference(&self, other: &BitvSet, f: |&uint| -> bool) -> bool {
        for (i, w1, w2) in self.commons(other) {
            if !iterate_bits(i, w1 & !w2, |b| f(&b)) {
                return false
            }
        };
        /* everything we have that they don't also shows up */
        self.outliers(other).advance(|(mine, i, w)|
            !mine || iterate_bits(i, w, |b| f(&b))
        )
    }

    pub fn symmetric_difference(&self, other: &BitvSet, f: |&uint| -> bool)
                                -> bool {
        for (i, w1, w2) in self.commons(other) {
            if !iterate_bits(i, w1 ^ w2, |b| f(&b)) {
                return false
            }
        };
        self.outliers(other).advance(|(_, i, w)| iterate_bits(i, w, |b| f(&b)))
    }

    pub fn intersection(&self, other: &BitvSet, f: |&uint| -> bool) -> bool {
        self.commons(other).advance(|(i, w1, w2)| iterate_bits(i, w1 & w2, |b| f(&b)))
    }

    pub fn union(&self, other: &BitvSet, f: |&uint| -> bool) -> bool {
        for (i, w1, w2) in self.commons(other) {
            if !iterate_bits(i, w1 | w2, |b| f(&b)) {
                return false
            }
        };
        self.outliers(other).advance(|(_, i, w)| iterate_bits(i, w, |b| f(&b)))
    }
}

impl fmt::Show for BitvSet {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(fmt, "{{"));
        let mut first = true;
        for n in self.iter() {
            if !first {
                try!(write!(fmt, ", "));
            }
            try!(write!(fmt, "{}", n));
            first = false;
        }
        write!(fmt, "}}")
    }
}

impl<S: hash::Writer> hash::Hash<S> for BitvSet {
    fn hash(&self, state: &mut S) {
        for pos in self.iter() {
            pos.hash(state);
        }
    }
}

impl Collection for BitvSet {
    #[inline]
    fn len(&self) -> uint  {
        let &BitvSet(ref bitv) = self;
        bitv.storage.iter().fold(0, |acc, &n| acc + n.count_ones())
    }
}

impl Mutable for BitvSet {
    fn clear(&mut self) {
        let &BitvSet(ref mut bitv) = self;
        bitv.clear();
    }
}

impl Set<uint> for BitvSet {
    fn contains(&self, value: &uint) -> bool {
        let &BitvSet(ref bitv) = self;
        *value < bitv.nbits && bitv.get(*value)
    }

    fn is_disjoint(&self, other: &BitvSet) -> bool {
        self.intersection(other, |_| false)
    }

    fn is_subset(&self, other: &BitvSet) -> bool {
        for (_, w1, w2) in self.commons(other) {
            if w1 & w2 != w1 {
                return false;
            }
        }
        /* If anything is not ours, then everything is not ours so we're
           definitely a subset in that case. Otherwise if there's any stray
           ones that 'other' doesn't have, we're not a subset. */
        for (mine, _, w) in self.outliers(other) {
            if !mine {
                return true;
            } else if w != 0 {
                return false;
            }
        }
        return true;
    }

    fn is_superset(&self, other: &BitvSet) -> bool {
        other.is_subset(self)
    }
}

impl MutableSet<uint> for BitvSet {
    fn insert(&mut self, value: uint) -> bool {
        if self.contains(&value) {
            return false;
        }
        if value >= self.capacity() {
            let new_cap = cmp::max(value + 1, self.capacity() * 2);
            self.grow(new_cap);
        }
        let &BitvSet(ref mut bitv) = self;
        if value >= bitv.nbits {
            // If we are increasing nbits, make sure we mask out any previously-unconsidered bits
            let old_rem = bitv.nbits % uint::BITS;
            if old_rem != 0 {
                let old_last_word = (bitv.nbits + uint::BITS - 1) / uint::BITS - 1;
                *bitv.storage.get_mut(old_last_word) &= (1 << old_rem) - 1;
            }
            bitv.nbits = value + 1;
        }
        bitv.set(value, true);
        return true;
    }

    fn remove(&mut self, value: &uint) -> bool {
        if !self.contains(value) {
            return false;
        }
        let &BitvSet(ref mut bitv) = self;
        bitv.set(*value, false);
        return true;
    }
}

impl BitvSet {
    /// Visits each of the words that the two bit vectors (`self` and `other`)
    /// both have in common. The three yielded arguments are (bit location,
    /// w1, w2) where the bit location is the number of bits offset so far,
    /// and w1/w2 are the words coming from the two vectors self, other.
    fn commons<'a>(&'a self, other: &'a BitvSet)
        -> Map<((uint, uint), (uint, uint)), (uint, uint, uint),
               Zip<MaskWords<'a>, MaskWords<'a>>> {
        let &BitvSet(ref self_bitv) = self;
        let &BitvSet(ref other_bitv) = other;
        self_bitv.mask_words(0).zip(other_bitv.mask_words(0))
            .map(|((i, w1), (_, w2))| (i * uint::BITS, w1, w2))
    }

    /// Visits each word in `self` or `other` that extends beyond the other. This
    /// will only iterate through one of the vectors, and it only iterates
    /// over the portion that doesn't overlap with the other one.
    ///
    /// The yielded arguments are a `bool`, the bit offset, and a word. The `bool`
    /// is true if the word comes from `self`, and `false` if it comes from
    /// `other`.
    fn outliers<'a>(&'a self, other: &'a BitvSet)
        -> Map<(uint, uint), (bool, uint, uint), MaskWords<'a>> {
        let slen = self.capacity() / uint::BITS;
        let olen = other.capacity() / uint::BITS;
        let &BitvSet(ref self_bitv) = self;
        let &BitvSet(ref other_bitv) = other;

        if olen < slen {
            self_bitv.mask_words(olen)
                .map(|(i, w)| (true, i * uint::BITS, w))
        } else {
            other_bitv.mask_words(slen)
                .map(|(i, w)| (false, i * uint::BITS, w))
        }
    }
}

pub struct BitPositions<'a> {
    set: &'a BitvSet,
    next_idx: uint
}

impl<'a> Iterator<uint> for BitPositions<'a> {
    #[inline]
    fn next(&mut self) -> Option<uint> {
        while self.next_idx < self.set.capacity() {
            let idx = self.next_idx;
            self.next_idx += 1;

            if self.set.contains(&idx) {
                return Some(idx);
            }
        }

        return None;
    }

    fn size_hint(&self) -> (uint, Option<uint>) {
        (0, Some(self.set.capacity() - self.next_idx))
    }
}

#[cfg(test)]
mod tests {
    use std::prelude::*;
    use std::uint;
    use std::rand;
    use std::rand::Rng;
    use test::Bencher;

    use {Set, Mutable, MutableSet};
    use bitv::{Bitv, BitvSet, from_bools, from_fn, from_bytes};
    use bitv;
    use vec::Vec;

    static BENCH_BITS : uint = 1 << 14;

    #[test]
    fn test_to_str() {
        let zerolen = Bitv::new(0u, false);
        assert_eq!(zerolen.to_str().as_slice(), "");

        let eightbits = Bitv::new(8u, false);
        assert_eq!(eightbits.to_str().as_slice(), "00000000")
    }

    #[test]
    fn test_0_elements() {
        let act = Bitv::new(0u, false);
        let exp = Vec::from_elem(0u, false);
        assert!(act.eq_vec(exp.as_slice()));
    }

    #[test]
    fn test_1_element() {
        let mut act = Bitv::new(1u, false);
        assert!(act.eq_vec([false]));
        act = Bitv::new(1u, true);
        assert!(act.eq_vec([true]));
    }

    #[test]
    fn test_2_elements() {
        let mut b = bitv::Bitv::new(2, false);
        b.set(0, true);
        b.set(1, false);
        assert_eq!(b.to_str().as_slice(), "10");
    }

    #[test]
    fn test_10_elements() {
        let mut act;
        // all 0

        act = Bitv::new(10u, false);
        assert!((act.eq_vec(
                    [false, false, false, false, false, false, false, false, false, false])));
        // all 1

        act = Bitv::new(10u, true);
        assert!((act.eq_vec([true, true, true, true, true, true, true, true, true, true])));
        // mixed

        act = Bitv::new(10u, false);
        act.set(0u, true);
        act.set(1u, true);
        act.set(2u, true);
        act.set(3u, true);
        act.set(4u, true);
        assert!((act.eq_vec([true, true, true, true, true, false, false, false, false, false])));
        // mixed

        act = Bitv::new(10u, false);
        act.set(5u, true);
        act.set(6u, true);
        act.set(7u, true);
        act.set(8u, true);
        act.set(9u, true);
        assert!((act.eq_vec([false, false, false, false, false, true, true, true, true, true])));
        // mixed

        act = Bitv::new(10u, false);
        act.set(0u, true);
        act.set(3u, true);
        act.set(6u, true);
        act.set(9u, true);
        assert!((act.eq_vec([true, false, false, true, false, false, true, false, false, true])));
    }

    #[test]
    fn test_31_elements() {
        let mut act;
        // all 0

        act = Bitv::new(31u, false);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false]));
        // all 1

        act = Bitv::new(31u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true]));
        // mixed

        act = Bitv::new(31u, false);
        act.set(0u, true);
        act.set(1u, true);
        act.set(2u, true);
        act.set(3u, true);
        act.set(4u, true);
        act.set(5u, true);
        act.set(6u, true);
        act.set(7u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(31u, false);
        act.set(16u, true);
        act.set(17u, true);
        act.set(18u, true);
        act.set(19u, true);
        act.set(20u, true);
        act.set(21u, true);
        act.set(22u, true);
        act.set(23u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, true, true, true, true, true, true, true,
                false, false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(31u, false);
        act.set(24u, true);
        act.set(25u, true);
        act.set(26u, true);
        act.set(27u, true);
        act.set(28u, true);
        act.set(29u, true);
        act.set(30u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, true, true, true, true, true, true, true]));
        // mixed

        act = Bitv::new(31u, false);
        act.set(3u, true);
        act.set(17u, true);
        act.set(30u, true);
        assert!(act.eq_vec(
                [false, false, false, true, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, false, false, false, false, false, false,
                false, false, false, false, false, false, true]));
    }

    #[test]
    fn test_32_elements() {
        let mut act;
        // all 0

        act = Bitv::new(32u, false);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false]));
        // all 1

        act = Bitv::new(32u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true]));
        // mixed

        act = Bitv::new(32u, false);
        act.set(0u, true);
        act.set(1u, true);
        act.set(2u, true);
        act.set(3u, true);
        act.set(4u, true);
        act.set(5u, true);
        act.set(6u, true);
        act.set(7u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(32u, false);
        act.set(16u, true);
        act.set(17u, true);
        act.set(18u, true);
        act.set(19u, true);
        act.set(20u, true);
        act.set(21u, true);
        act.set(22u, true);
        act.set(23u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, true, true, true, true, true, true, true,
                false, false, false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(32u, false);
        act.set(24u, true);
        act.set(25u, true);
        act.set(26u, true);
        act.set(27u, true);
        act.set(28u, true);
        act.set(29u, true);
        act.set(30u, true);
        act.set(31u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, true, true, true, true, true, true, true, true]));
        // mixed

        act = Bitv::new(32u, false);
        act.set(3u, true);
        act.set(17u, true);
        act.set(30u, true);
        act.set(31u, true);
        assert!(act.eq_vec(
                [false, false, false, true, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, false, false, false, false, false, false,
                false, false, false, false, false, false, true, true]));
    }

    #[test]
    fn test_33_elements() {
        let mut act;
        // all 0

        act = Bitv::new(33u, false);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false]));
        // all 1

        act = Bitv::new(33u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true, true, true, true, true, true, true, true, true, true,
                true, true, true, true, true, true]));
        // mixed

        act = Bitv::new(33u, false);
        act.set(0u, true);
        act.set(1u, true);
        act.set(2u, true);
        act.set(3u, true);
        act.set(4u, true);
        act.set(5u, true);
        act.set(6u, true);
        act.set(7u, true);
        assert!(act.eq_vec(
                [true, true, true, true, true, true, true, true, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(33u, false);
        act.set(16u, true);
        act.set(17u, true);
        act.set(18u, true);
        act.set(19u, true);
        act.set(20u, true);
        act.set(21u, true);
        act.set(22u, true);
        act.set(23u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, true, true, true, true, true, true, true,
                false, false, false, false, false, false, false, false, false]));
        // mixed

        act = Bitv::new(33u, false);
        act.set(24u, true);
        act.set(25u, true);
        act.set(26u, true);
        act.set(27u, true);
        act.set(28u, true);
        act.set(29u, true);
        act.set(30u, true);
        act.set(31u, true);
        assert!(act.eq_vec(
                [false, false, false, false, false, false, false, false, false, false, false,
                false, false, false, false, false, false, false, false, false, false, false, false,
                false, true, true, true, true, true, true, true, true, false]));
        // mixed

        act = Bitv::new(33u, false);
        act.set(3u, true);
        act.set(17u, true);
        act.set(30u, true);
        act.set(31u, true);
        act.set(32u, true);
        assert!(act.eq_vec(
                [false, false, false, true, false, false, false, false, false, false, false, false,
                false, false, false, false, false, true, false, false, false, false, false, false,
                false, false, false, false, false, false, true, true, true]));
    }

    #[test]
    fn test_equal_differing_sizes() {
        let v0 = Bitv::new(10u, false);
        let v1 = Bitv::new(11u, false);
        assert!(v0 != v1);
    }

    #[test]
    fn test_equal_greatly_differing_sizes() {
        let v0 = Bitv::new(10u, false);
        let v1 = Bitv::new(110u, false);
        assert!(v0 != v1);
    }

    #[test]
    fn test_equal_sneaky_small() {
        let mut a = bitv::Bitv::new(1, false);
        a.set(0, true);

        let mut b = bitv::Bitv::new(1, true);
        b.set(0, true);

        assert_eq!(a, b);
    }

    #[test]
    fn test_equal_sneaky_big() {
        let mut a = bitv::Bitv::new(100, false);
        for i in range(0u, 100) {
            a.set(i, true);
        }

        let mut b = bitv::Bitv::new(100, true);
        for i in range(0u, 100) {
            b.set(i, true);
        }

        assert_eq!(a, b);
    }

    #[test]
    fn test_from_bytes() {
        let bitv = from_bytes([0b10110110, 0b00000000, 0b11111111]);
        let str = format!("{}{}{}", "10110110", "00000000", "11111111");
        assert_eq!(bitv.to_str().as_slice(), str.as_slice());
    }

    #[test]
    fn test_to_bytes() {
        let mut bv = Bitv::new(3, true);
        bv.set(1, false);
        assert_eq!(bv.to_bytes(), vec!(0b10100000));

        let mut bv = Bitv::new(9, false);
        bv.set(2, true);
        bv.set(8, true);
        assert_eq!(bv.to_bytes(), vec!(0b00100000, 0b10000000));
    }

    #[test]
    fn test_from_bools() {
        assert!(from_bools([true, false, true, true]).to_str().as_slice() ==
                "1011");
    }

    #[test]
    fn test_to_bools() {
        let bools = vec!(false, false, true, false, false, true, true, false);
        assert_eq!(from_bytes([0b00100110]).iter().collect::<Vec<bool>>(), bools);
    }

    #[test]
    fn test_bitv_iterator() {
        let bools = [true, false, true, true];
        let bitv = from_bools(bools);

        for (act, &ex) in bitv.iter().zip(bools.iter()) {
            assert_eq!(ex, act);
        }
    }

    #[test]
    fn test_bitv_set_iterator() {
        let bools = [true, false, true, true];
        let bitv = BitvSet::from_bitv(from_bools(bools));

        let idxs: Vec<uint> = bitv.iter().collect();
        assert_eq!(idxs, vec!(0, 2, 3));
    }

    #[test]
    fn test_bitv_set_frombitv_init() {
        let bools = [true, false];
        let lengths = [10, 64, 100];
        for &b in bools.iter() {
            for &l in lengths.iter() {
                let bitset = BitvSet::from_bitv(Bitv::new(l, b));
                assert_eq!(bitset.contains(&1u), b)
                assert_eq!(bitset.contains(&(l-1u)), b)
                assert!(!bitset.contains(&l))
            }
        }
    }

    #[test]
    fn test_small_difference() {
        let mut b1 = Bitv::new(3, false);
        let mut b2 = Bitv::new(3, false);
        b1.set(0, true);
        b1.set(1, true);
        b2.set(1, true);
        b2.set(2, true);
        assert!(b1.difference(&b2));
        assert!(b1[0]);
        assert!(!b1[1]);
        assert!(!b1[2]);
    }

    #[test]
    fn test_big_difference() {
        let mut b1 = Bitv::new(100, false);
        let mut b2 = Bitv::new(100, false);
        b1.set(0, true);
        b1.set(40, true);
        b2.set(40, true);
        b2.set(80, true);
        assert!(b1.difference(&b2));
        assert!(b1[0]);
        assert!(!b1[40]);
        assert!(!b1[80]);
    }

    #[test]
    fn test_small_clear() {
        let mut b = Bitv::new(14, true);
        b.clear();
        BitvSet::from_bitv(b).iter().advance(|i| {
            fail!("found 1 at {:?}", i)
        });
    }

    #[test]
    fn test_big_clear() {
        let mut b = Bitv::new(140, true);
        b.clear();
        BitvSet::from_bitv(b).iter().advance(|i| {
            fail!("found 1 at {:?}", i)
        });
    }

    #[test]
    fn test_bitv_masking() {
        let b = Bitv::new(140, true); 
        let mut bs = BitvSet::from_bitv(b);
        assert!(bs.contains(&139));
        assert!(!bs.contains(&140));
        assert!(bs.insert(150));
        assert!(!bs.contains(&140));
        assert!(!bs.contains(&149));
        assert!(bs.contains(&150));
        assert!(!bs.contains(&151));
    }

    #[test]
    fn test_bitv_set_basic() {
        // calculate nbits with uint::BITS granularity
        fn calc_nbits(bits: uint) -> uint {
            uint::BITS * ((bits + uint::BITS - 1) / uint::BITS)
        }

        let mut b = BitvSet::new();
        assert_eq!(b.capacity(), calc_nbits(0));
        assert!(b.insert(3));
        assert_eq!(b.capacity(), calc_nbits(3));
        assert!(!b.insert(3));
        assert!(b.contains(&3));
        assert!(b.insert(4));
        assert!(!b.insert(4));
        assert!(b.contains(&3));
        assert!(b.insert(400));
        assert_eq!(b.capacity(), calc_nbits(400));
        assert!(!b.insert(400));
        assert!(b.contains(&400));
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn test_bitv_set_intersection() {
        let mut a = BitvSet::new();
        let mut b = BitvSet::new();

        assert!(a.insert(11));
        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(77));
        assert!(a.insert(103));
        assert!(a.insert(5));

        assert!(b.insert(2));
        assert!(b.insert(11));
        assert!(b.insert(77));
        assert!(b.insert(5));
        assert!(b.insert(3));

        let mut i = 0;
        let expected = [3, 5, 11, 77];
        a.intersection(&b, |x| {
            assert_eq!(*x, expected[i]);
            i += 1;
            true
        });
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_bitv_set_difference() {
        let mut a = BitvSet::new();
        let mut b = BitvSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(200));
        assert!(a.insert(500));

        assert!(b.insert(3));
        assert!(b.insert(200));

        let mut i = 0;
        let expected = [1, 5, 500];
        a.difference(&b, |x| {
            assert_eq!(*x, expected[i]);
            i += 1;
            true
        });
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_bitv_set_symmetric_difference() {
        let mut a = BitvSet::new();
        let mut b = BitvSet::new();

        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));

        assert!(b.insert(3));
        assert!(b.insert(9));
        assert!(b.insert(14));
        assert!(b.insert(220));

        let mut i = 0;
        let expected = [1, 5, 11, 14, 220];
        a.symmetric_difference(&b, |x| {
            assert_eq!(*x, expected[i]);
            i += 1;
            true
        });
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_bitv_set_union() {
        let mut a = BitvSet::new();
        let mut b = BitvSet::new();
        assert!(a.insert(1));
        assert!(a.insert(3));
        assert!(a.insert(5));
        assert!(a.insert(9));
        assert!(a.insert(11));
        assert!(a.insert(160));
        assert!(a.insert(19));
        assert!(a.insert(24));

        assert!(b.insert(1));
        assert!(b.insert(5));
        assert!(b.insert(9));
        assert!(b.insert(13));
        assert!(b.insert(19));

        let mut i = 0;
        let expected = [1, 3, 5, 9, 11, 13, 19, 24, 160];
        a.union(&b, |x| {
            assert_eq!(*x, expected[i]);
            i += 1;
            true
        });
        assert_eq!(i, expected.len());
    }

    #[test]
    fn test_bitv_remove() {
        let mut a = BitvSet::new();

        assert!(a.insert(1));
        assert!(a.remove(&1));

        assert!(a.insert(100));
        assert!(a.remove(&100));

        assert!(a.insert(1000));
        assert!(a.remove(&1000));
        a.shrink_to_fit();
        assert_eq!(a.capacity(), uint::BITS);
    }

    #[test]
    fn test_bitv_clone() {
        let mut a = BitvSet::new();

        assert!(a.insert(1));
        assert!(a.insert(100));
        assert!(a.insert(1000));

        let mut b = a.clone();

        assert!(a == b);

        assert!(b.remove(&1));
        assert!(a.contains(&1));

        assert!(a.remove(&1000));
        assert!(b.contains(&1000));
    }

    #[test]
    fn test_small_bitv_tests() {
        let v = from_bytes([0]);
        assert!(!v.all());
        assert!(!v.any());
        assert!(v.none());

        let v = from_bytes([0b00010100]);
        assert!(!v.all());
        assert!(v.any());
        assert!(!v.none());

        let v = from_bytes([0xFF]);
        assert!(v.all());
        assert!(v.any());
        assert!(!v.none());
    }

    #[test]
    fn test_big_bitv_tests() {
        let v = from_bytes([ // 88 bits
            0, 0, 0, 0,
            0, 0, 0, 0,
            0, 0, 0]);
        assert!(!v.all());
        assert!(!v.any());
        assert!(v.none());

        let v = from_bytes([ // 88 bits
            0, 0, 0b00010100, 0,
            0, 0, 0, 0b00110100,
            0, 0, 0]);
        assert!(!v.all());
        assert!(v.any());
        assert!(!v.none());

        let v = from_bytes([ // 88 bits
            0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF]);
        assert!(v.all());
        assert!(v.any());
        assert!(!v.none());
    }

    #[test]
    fn test_bitv_set_show() {
        let mut s = BitvSet::new();
        s.insert(1);
        s.insert(10);
        s.insert(50);
        s.insert(2);
        assert_eq!("{1, 2, 10, 50}".to_string(), s.to_str());
    }

    fn rng() -> rand::IsaacRng {
        let seed = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
        rand::SeedableRng::from_seed(seed)
    }

    #[bench]
    fn bench_uint_small(b: &mut Bencher) {
        let mut r = rng();
        let mut bitv = 0 as uint;
        b.iter(|| {
            bitv |= 1 << ((r.next_u32() as uint) % uint::BITS);
            &bitv
        })
    }

    #[bench]
    fn bench_bitv_big(b: &mut Bencher) {
        let mut r = rng();
        let mut bitv = Bitv::new(BENCH_BITS, false);
        b.iter(|| {
            bitv.set((r.next_u32() as uint) % BENCH_BITS, true);
            &bitv
        })
    }

    #[bench]
    fn bench_bitv_small(b: &mut Bencher) {
        let mut r = rng();
        let mut bitv = Bitv::new(uint::BITS, false);
        b.iter(|| {
            bitv.set((r.next_u32() as uint) % uint::BITS, true);
            &bitv
        })
    }

    #[bench]
    fn bench_bitv_set_small(b: &mut Bencher) {
        let mut r = rng();
        let mut bitv = BitvSet::new();
        b.iter(|| {
            bitv.insert((r.next_u32() as uint) % uint::BITS);
            &bitv
        })
    }

    #[bench]
    fn bench_bitv_set_big(b: &mut Bencher) {
        let mut r = rng();
        let mut bitv = BitvSet::new();
        b.iter(|| {
            bitv.insert((r.next_u32() as uint) % BENCH_BITS);
            &bitv
        })
    }

    #[bench]
    fn bench_bitv_big_union(b: &mut Bencher) {
        let mut b1 = Bitv::new(BENCH_BITS, false);
        let b2 = Bitv::new(BENCH_BITS, false);
        b.iter(|| {
            b1.union(&b2);
        })
    }

    #[bench]
    fn bench_btv_small_iter(b: &mut Bencher) {
        let bitv = Bitv::new(uint::BITS, false);
        b.iter(|| {
            let mut _sum = 0;
            for pres in bitv.iter() {
                _sum += pres as uint;
            }
        })
    }

    #[bench]
    fn bench_bitv_big_iter(b: &mut Bencher) {
        let bitv = Bitv::new(BENCH_BITS, false);
        b.iter(|| {
            let mut _sum = 0;
            for pres in bitv.iter() {
                _sum += pres as uint;
            }
        })
    }

    #[bench]
    fn bench_bitvset_iter(b: &mut Bencher) {
        let bitv = BitvSet::from_bitv(from_fn(BENCH_BITS,
                                              |idx| {idx % 3 == 0}));
        b.iter(|| {
            let mut _sum = 0;
            for idx in bitv.iter() {
                _sum += idx;
            }
        })
    }
}
