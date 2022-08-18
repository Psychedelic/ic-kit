use crate::{AsHashTree, Hash, HashTree};
use candid::types::Type;
use candid::CandidType;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use std::borrow::Borrow;
use std::iter::FromIterator;
use std::ops::Index;
use std::slice::{Iter, SliceIndex};

/// An append only list of `T`.
///
/// # Example
///
/// ```
/// use ic_kit_certified::Seq;
///
/// let mut seq = Seq::<u8>::new();
///
/// seq.append(0);
/// seq.append(1);
///
/// assert_eq!(seq.len(), 2);
/// ```
#[derive(Default, Eq, PartialEq, Clone, Debug)]
pub struct Seq<T> {
    hash: Hash,
    items: Vec<T>,
}

impl<T> Seq<T> {
    /// Create a new, empty Stack<T>
    #[inline]
    pub const fn new() -> Self {
        Self {
            items: Vec::new(),
            hash: [0; 32],
        }
    }

    /// Construct a new, empty Stack<T> with the specified capacity.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            hash: [0; 32],
        }
    }
}

impl<T: AsHashTree> Seq<T> {
    /// Append a new item to the sequence and update the hash.
    pub fn append(&mut self, item: T) {
        let mut h = Sha256::new();
        h.update(&self.hash);
        h.update(item.root_hash());

        self.hash = h.finalize().into();
        self.items.push(item);
    }

    /// Clear the sequence by removing all of the items. This method does not have
    /// any effects on the allocated memory.
    #[inline]
    pub fn clear(&mut self) {
        self.hash = [0; 32];
        self.items.clear();
    }

    /// Shrinks the capacity of the seq as much as possible.
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Reserve space for at least `additional` more elements.
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.items.reserve(additional)
    }

    /// Reserve space for exactly `additional` more elements.
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.items.reserve_exact(additional)
    }

    /// Return the underlying vector containing the items.
    #[inline]
    pub fn as_vec(&self) -> &Vec<T> {
        &self.items
    }

    /// Returns `true` if the sequence does not have any elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.items.len() == 0
    }

    /// Returns the number of elements in the sequence, also referred to as its ‘length’.
    #[inline]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns the number of elements the sequence can hold without reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    /// Returns an iterator over the data.
    #[inline]
    pub fn iter(&self) -> Iter<'_, T> {
        self.items.iter()
    }

    /// Recompute the hash of the sequence.
    #[inline]
    fn recompute_hash(&mut self, prev_len: usize) {
        let mut hash = self.hash;

        for item in &self.items[prev_len..] {
            let mut h = Sha256::new();
            h.update(&hash);
            h.update(item.root_hash());
            hash = h.finalize().into();
        }

        self.hash = hash;
    }
}

impl<T: AsHashTree> AsHashTree for Seq<T> {
    #[inline]
    fn root_hash(&self) -> Hash {
        self.hash
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Pruned(self.hash)
    }
}

impl<T: AsHashTree> From<Vec<T>> for Seq<T> {
    #[inline]
    fn from(items: Vec<T>) -> Self {
        let mut seq = Seq {
            items,
            hash: [0; 32],
        };

        seq.recompute_hash(0);

        seq
    }
}

impl<T: AsHashTree> FromIterator<T> for Seq<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut seq = Seq {
            items: iter.into_iter().collect(),
            hash: [0; 32],
        };

        seq.recompute_hash(0);

        seq
    }
}

impl<T: AsHashTree> From<Seq<T>> for Vec<T> {
    #[inline]
    fn from(seq: Seq<T>) -> Self {
        seq.items
    }
}

impl<T: AsHashTree> AsRef<[T]> for Seq<T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.items.as_ref()
    }
}

impl<T: AsHashTree> Borrow<[T]> for Seq<T> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.items.borrow()
    }
}

impl<'a, T: AsHashTree + Copy + 'a> Extend<&'a T> for Seq<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        let prev_len = self.items.len();
        self.items.extend(iter);
        self.recompute_hash(prev_len)
    }
}

impl<T: AsHashTree> Extend<T> for Seq<T> {
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        let prev_len = self.items.len();
        self.items.extend(iter);
        self.recompute_hash(prev_len)
    }
}

impl<'a, T: AsHashTree + Clone> From<&'a [T]> for Seq<T> {
    #[inline]
    fn from(items: &'a [T]) -> Self {
        let mut seq = Seq {
            items: items.into(),
            hash: [0; 32],
        };
        seq.recompute_hash(0);
        seq
    }
}

impl<'a, T: AsHashTree + Clone> From<&'a mut [T]> for Seq<T> {
    #[inline]
    fn from(items: &'a mut [T]) -> Self {
        let mut seq = Seq {
            items: items.into(),
            hash: [0; 32],
        };
        seq.recompute_hash(0);
        seq
    }
}

impl<T: AsHashTree, I: SliceIndex<[T]>> Index<I> for Seq<T> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        self.items.index(index)
    }
}

impl<T: Serialize + AsHashTree> Serialize for Seq<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.items.serialize(serializer)
    }
}

impl<'de, T: AsHashTree + Deserialize<'de>> Deserialize<'de> for Seq<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut seq = Seq {
            items: <Vec<T>>::deserialize(deserializer)?,
            hash: [0; 32],
        };

        seq.recompute_hash(0);

        Ok(seq)
    }
}

impl<T: CandidType> CandidType for Seq<T> {
    fn _ty() -> Type {
        <Vec<T>>::_ty()
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        self.items.idl_serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candid::{decode_one, encode_one};

    #[test]
    fn append() {
        let mut seq = Seq::<usize>::with_capacity(1000);
        let mut hash = seq.root_hash();
        assert_eq!(seq.is_empty(), true);

        for i in 0..1000 {
            seq.append(i);
            assert_eq!(seq.len(), i + 1);
            let new_hash = seq.root_hash();
            assert_ne!(hash, new_hash);
            hash = new_hash;
        }

        assert_eq!(seq.is_empty(), false);

        seq.clear();
        assert_eq!(seq.len(), 0);
        assert_eq!(seq.is_empty(), true);
        assert_eq!(seq.root_hash(), Seq::<usize>::new().root_hash());

        for i in 0..1000 {
            seq.append(i);
        }

        assert_eq!(hash, seq.root_hash());
    }

    #[test]
    fn extend() {
        let manual = {
            let mut seq = Seq::<usize>::with_capacity(100);
            for i in 0..100 {
                seq.append(i);
            }
            seq
        };

        {
            let mut seq = Seq::<usize>::new();
            seq.extend(0..100);

            assert_eq!(manual.len(), seq.len());
            assert_eq!(manual.root_hash(), seq.root_hash());
        }

        {
            let mut seq = Seq::<usize>::new();
            seq.extend(0..50);
            seq.extend(50..100);
            assert_eq!(manual.len(), seq.len());
            assert_eq!(manual.root_hash(), seq.root_hash());
        }

        {
            let mut seq = Seq::<usize>::new();
            seq.extend(0..50);
            seq.append(50);
            seq.extend(51..100);
            assert_eq!(manual.len(), seq.len());
            assert_eq!(manual.root_hash(), seq.root_hash());
        }
    }

    #[test]
    fn index() {
        let seq = (0..100).collect::<Seq<_>>();

        for i in 0..100 {
            assert_eq!(seq[i], i);
        }
    }

    #[test]
    #[should_panic]
    fn index_out_of_range() {
        let seq = Seq::<u8>::new();
        seq[0];
    }

    #[test]
    fn serde_cbor() {
        let seq = (0..10).collect::<Seq<_>>();
        let serialized = serde_cbor::to_vec(&seq).unwrap();
        let actual: Seq<i32> = serde_cbor::from_slice(&serialized).unwrap();
        assert_eq!(actual.len(), 10);
        assert_eq!(actual.hash, seq.hash);
        assert_eq!(actual, seq);
        let expected = (0..10).collect::<Vec<_>>();
        let deserialized_as_vec: Vec<i32> = serde_cbor::from_slice(&serialized).unwrap();
        assert_eq!(deserialized_as_vec, expected);
    }

    #[test]
    fn candid() {
        let seq = (0..10).collect::<Seq<_>>();
        let encoded = encode_one(&seq).unwrap();
        let decoded: Seq<i32> = decode_one(&encoded).unwrap();
        assert_eq!(seq, decoded);
    }
}
