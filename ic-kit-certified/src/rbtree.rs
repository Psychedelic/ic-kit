//! This file contains a low-level RbTree implementation. The code is borrowed from the
//! `ic-certified-map` crate by Dfinity.
//!
//! It is not recommend to use the [`RbTree`] directly since it is a low level data structure
//! and does only provide basic functionalities. Instead we advise you to look at the
//! [crate::collections] module.

use std::borrow::{Borrow, Cow};
use std::cmp::Ordering;
use std::cmp::Ordering::{Equal, Greater, Less};
use std::fmt;

use crate::hashtree::{
    fork, fork_hash, labeled_hash, Hash,
    HashTree::{self, Empty, Pruned},
};
use crate::label::{Label, Prefix};
use crate::AsHashTree;

#[cfg(test)]
pub(crate) mod debug_alloc;

pub mod entry;
pub mod iterator;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    Red,
    Black,
}

impl Color {
    fn flip(self) -> Self {
        match self {
            Self::Red => Self::Black,
            Self::Black => Self::Red,
        }
    }
}

impl<K: 'static + Label, V: AsHashTree + 'static> AsHashTree for RbTree<K, V> {
    #[inline]
    fn root_hash(&self) -> Hash {
        if self.root.is_null() {
            Empty.reconstruct()
        } else {
            unsafe { (*self.root).subtree_hash }
        }
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        unsafe { Node::full_witness_tree(self.root, Node::data_tree) }
    }
}

#[derive(PartialEq, Debug)]
enum KeyBound<'a, T: Label> {
    Exact(&'a T),
    Neighbor(&'a T),
}

impl<'a, T: Label> Clone for KeyBound<'a, T> {
    fn clone(&self) -> Self {
        match self {
            KeyBound::Exact(k) => KeyBound::Exact(*k),
            KeyBound::Neighbor(k) => KeyBound::Neighbor(*k),
        }
    }
}

impl<'a, T: Label> Copy for KeyBound<'a, T> {}

impl<'a, T: Label> Eq for KeyBound<'a, T> {}

impl<'a, T: Label> PartialOrd<Self> for KeyBound<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_ref().partial_cmp(other.as_ref())
    }
}

impl<'a, T: Label> Ord for KeyBound<'a, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ref().cmp(other.as_ref())
    }
}

impl<'a, T: Label> Label for KeyBound<'a, T> {
    fn as_label(&self) -> Cow<[u8]> {
        match self {
            KeyBound::Exact(key) => key.as_label(),
            KeyBound::Neighbor(key) => key.as_label(),
        }
    }
}

impl<'a, T: Label> AsRef<T> for KeyBound<'a, T> {
    fn as_ref(&self) -> &T {
        match self {
            KeyBound::Exact(key) => key,
            KeyBound::Neighbor(key) => key,
        }
    }
}

impl<'a, T: Label + AsRef<[u8]>> AsRef<[u8]> for KeyBound<'a, T> {
    fn as_ref(&self) -> &[u8] {
        match self {
            KeyBound::Exact(key) => key.as_ref(),
            KeyBound::Neighbor(key) => key.as_ref(),
        }
    }
}

// 1. All leaves are black.
// 2. Children of a red node are black.
// 3. Every path from a node goes through the same number of black
//    nodes.
struct Node<K, V> {
    key: K,
    value: V,
    left: *mut Node<K, V>,
    right: *mut Node<K, V>,
    color: Color,

    /// Hash of the full hash tree built from this node and its
    /// children. It needs to be recomputed after every rotation.
    subtree_hash: Hash,
}

impl<K: 'static + Label, V: AsHashTree + 'static> Node<K, V> {
    #[allow(clippy::let_and_return)]
    fn new(key: K, value: V) -> *mut Self {
        let value_hash = value.root_hash();
        let data_hash = labeled_hash(&key.as_label(), &value_hash);
        let node = Box::into_raw(Box::new(Self {
            key,
            value,
            left: Node::null(),
            right: Node::null(),
            color: Color::Red,
            subtree_hash: data_hash,
        }));

        #[cfg(test)]
        debug_alloc::mark_pointer_allocated(node);

        node
    }

    unsafe fn data_hash(n: *mut Self) -> Hash {
        debug_assert!(!n.is_null());
        labeled_hash(&(*n).key.as_label(), &(*n).value.root_hash())
    }

    unsafe fn left_hash_tree<'a>(n: *mut Self) -> HashTree<'a> {
        debug_assert!(!n.is_null());
        if (*n).left.is_null() {
            Empty
        } else {
            Pruned((*(*n).left).subtree_hash)
        }
    }

    unsafe fn right_hash_tree<'a>(n: *mut Self) -> HashTree<'a> {
        debug_assert!(!n.is_null());
        if (*n).right.is_null() {
            Empty
        } else {
            Pruned((*(*n).right).subtree_hash)
        }
    }

    fn null() -> *mut Self {
        std::ptr::null::<Self>() as *mut Node<K, V>
    }

    unsafe fn data_tree<'a>(n: *mut Self) -> HashTree<'a> {
        debug_assert!(!n.is_null());
        HashTree::Labeled((*n).key.as_label(), Box::new((*n).value.as_hash_tree()))
    }

    unsafe fn subtree_with<'a>(
        n: *mut Self,
        f: impl FnOnce(&'a V) -> HashTree<'a>,
    ) -> HashTree<'a> {
        debug_assert!(!n.is_null());

        HashTree::Labeled((*n).key.as_label(), Box::new(f(&(*n).value)))
    }

    unsafe fn witness_tree<'a>(n: *mut Self) -> HashTree<'a> {
        debug_assert!(!n.is_null());
        let value_hash = (*n).value.root_hash();
        HashTree::Labeled((*n).key.as_label(), Box::new(Pruned(value_hash)))
    }

    unsafe fn full_witness_tree<'a>(
        n: *mut Self,
        f: unsafe fn(*mut Self) -> HashTree<'a>,
    ) -> HashTree<'a> {
        if n.is_null() {
            return Empty;
        }
        three_way_fork(
            Self::full_witness_tree((*n).left, f),
            f(n),
            Self::full_witness_tree((*n).right, f),
        )
    }

    unsafe fn delete(n: *mut Self) -> Option<(K, V)> {
        if n.is_null() {
            return None;
        }
        Self::delete((*n).left);
        Self::delete((*n).right);
        let node = Box::from_raw(n);

        #[cfg(test)]
        debug_alloc::mark_pointer_deleted(n);

        Some((node.key, node.value))
    }

    unsafe fn subtree_hash(n: *mut Self) -> Hash {
        if n.is_null() {
            return Empty.reconstruct();
        }

        let h = Node::data_hash(n);

        match ((*n).left.is_null(), (*n).right.is_null()) {
            (true, true) => h,
            (false, true) => fork_hash(&(*(*n).left).subtree_hash, &h),
            (true, false) => fork_hash(&h, &(*(*n).right).subtree_hash),
            (false, false) => fork_hash(
                &(*(*n).left).subtree_hash,
                &fork_hash(&h, &(*(*n).right).subtree_hash),
            ),
        }
    }
}

/// Implements mutable Leaf-leaning red-black trees as defined in
/// https://www.cs.princeton.edu/~rs/talks/LLRB/LLRB.pdf
pub struct RbTree<K: 'static + Label, V: AsHashTree + 'static> {
    len: usize,
    root: *mut Node<K, V>,
}

impl<K: 'static + Label, V: AsHashTree + 'static> Drop for RbTree<K, V> {
    fn drop(&mut self) {
        unsafe {
            Node::delete(self.root);
        }
    }
}

impl<K: 'static + Label, V: AsHashTree + 'static> Default for RbTree<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: 'static + Label, V: AsHashTree + 'static> RbTree<K, V> {
    #[inline]
    pub fn new() -> Self {
        Self {
            len: 0,
            root: Node::null(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.root.is_null()
    }

    pub fn entry(&mut self, key: K) -> entry::Entry<K, V> {
        let node = unsafe { self.get_node(&key) };

        if node.is_null() {
            entry::Entry::Vacant(entry::VacantEntry { map: self, key })
        } else {
            entry::Entry::Occupied(entry::OccupiedEntry {
                map: self,
                key,
                node,
            })
        }
    }

    #[inline]
    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe {
            let mut root = self.root;
            while !root.is_null() {
                match key.cmp((*root).key.borrow()) {
                    Equal => return Some(&(*root).value),
                    Less => root = (*root).left,
                    Greater => root = (*root).right,
                }
            }
            None
        }
    }

    #[inline]
    pub fn get_with(&self, cmp: impl Fn(&K) -> Ordering) -> Option<&V> {
        unsafe {
            let mut root = self.root;
            while !root.is_null() {
                match cmp(&(*root).key) {
                    Equal => return Some(&(*root).value),
                    Less => root = (*root).left,
                    Greater => root = (*root).right,
                }
            }
            None
        }
    }

    #[inline]
    unsafe fn get_node(&self, key: &K) -> *mut Node<K, V> {
        let mut root = self.root;
        while !root.is_null() {
            match key.cmp(&(*root).key) {
                Equal => return root,
                Less => root = (*root).left,
                Greater => root = (*root).right,
            }
        }
        Node::null()
    }

    /// Updates the value corresponding to the specified key.
    #[inline]
    pub fn modify<'a, Q: ?Sized, T>(&mut self, key: &Q, f: impl FnOnce(&'a mut V) -> T) -> Option<T>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V: AsHashTree + 'static, T, Q: ?Sized>(
            mut h: *mut Node<K, V>,
            k: &Q,
            f: impl FnOnce(&'a mut V) -> T,
        ) -> Option<T>
        where
            K: Borrow<Q>,
            Q: Ord,
        {
            if h.is_null() {
                return None;
            }

            match k.cmp((*h).key.borrow()) {
                Equal => {
                    let res = f(&mut (*h).value);
                    (*h).subtree_hash = Node::subtree_hash(h);
                    Some(res)
                }
                Less => {
                    let res = go((*h).left, k, f);
                    (*h).subtree_hash = Node::subtree_hash(h);
                    res
                }
                Greater => {
                    let res = go((*h).right, k, f);
                    (*h).subtree_hash = Node::subtree_hash(h);
                    res
                }
            }
        }
        unsafe { go(self.root, key, f) }
    }

    /// Modify the maximum node with the given prefix.
    pub fn modify_max_with_prefix<'a, P: ?Sized, T>(
        &mut self,
        prefix: &P,
        f: impl FnOnce(&'a K, &'a mut V) -> T,
    ) -> Option<T>
    where
        K: Prefix<P>,
        P: Ord,
    {
        unsafe fn go<
            'a,
            K: Label + 'static,
            V: AsHashTree + 'static,
            P: ?Sized,
            T,
            F: FnOnce(&'a K, &'a mut V) -> T,
        >(
            mut h: *mut Node<K, V>,
            prefix: &P,
            f: F,
        ) -> (Option<T>, Option<F>)
        where
            K: Prefix<P>,
            P: Ord,
        {
            if h.is_null() {
                return (None, Some(f));
            }

            let node_key = &(*h).key;
            let key_prefix = node_key.borrow();

            let res = match key_prefix.cmp(prefix) {
                Greater | Equal if node_key.is_prefix(prefix) => match go((*h).right, prefix, f) {
                    (None, Some(f)) => {
                        let ret = f(node_key, &mut (*h).value);
                        (Some(ret), None)
                    }
                    ret => ret,
                },
                Greater => go((*h).left, prefix, f),
                Less | Equal => go((*h).right, prefix, f),
            };

            if res.0.is_some() {
                (*h).subtree_hash = Node::subtree_hash(h);
            }

            res
        }

        unsafe { go(self.root, prefix, f).0 }
    }

    pub fn max_entry_with_prefix<P: ?Sized>(&self, prefix: &P) -> Option<(&K, &V)>
    where
        K: Prefix<P>,
        P: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V, P: ?Sized>(
            n: *mut Node<K, V>,
            prefix: &P,
        ) -> Option<(&'a K, &'a V)>
        where
            K: Prefix<P>,
            P: Ord,
        {
            if n.is_null() {
                return None;
            }

            let node_key = &(*n).key;
            let key_prefix = node_key.borrow();
            match key_prefix.cmp(prefix) {
                Greater | Equal if node_key.is_prefix(prefix) => {
                    go((*n).right, prefix).or(Some((node_key, &(*n).value)))
                }
                Greater => go((*n).left, prefix),
                Less | Equal => go((*n).right, prefix),
            }
        }
        unsafe { go(self.root, prefix) }
    }

    fn range_witness<'a>(
        &'a self,
        left: Option<KeyBound<'a, K>>,
        right: Option<KeyBound<'a, K>>,
        f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
    ) -> HashTree<'a> {
        match (left, right) {
            (None, None) => unsafe { Node::full_witness_tree(self.root, f) },
            (Some(l), None) => self.witness_range_above(l, f),
            (None, Some(r)) => self.witness_range_below(r, f),
            (Some(l), Some(r)) => self.witness_range_between(l, r, f),
        }
    }

    /// Constructs a hash tree that acts as a proof that there is a
    /// entry with the specified key in this map.  The proof also
    /// contains the value in question.
    ///
    /// If the key is not in the map, returns a proof of absence.
    #[inline]
    pub fn witness<Q: ?Sized>(&self, key: &Q) -> HashTree<'_>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        self.nested_witness(key, |v| v.as_hash_tree())
    }

    /// Like `witness`, but gives the caller more control over the
    /// construction of the value witness.  This method is useful for
    /// constructing witnesses for nested certified maps.
    #[inline]
    pub fn nested_witness<'a, Q: ?Sized>(
        &'a self,
        key: &Q,
        f: impl FnOnce(&'a V) -> HashTree<'a>,
    ) -> HashTree<'a>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        if let Some(t) = self.lookup_and_build_witness(key, f) {
            return t;
        }
        self.range_witness(
            self.lower_bound(key),
            self.upper_bound(key),
            Node::witness_tree,
        )
    }

    /// Returns a witness enumerating all the keys in this map.  The
    /// resulting tree doesn't include values, they are replaced with
    /// "Pruned" nodes.
    #[inline]
    pub fn keys(&self) -> HashTree<'_> {
        unsafe { Node::full_witness_tree(self.root, Node::witness_tree) }
    }

    /// Returns a witness for the keys in the specified range.  The
    /// resulting tree doesn't include values, they are replaced with
    /// "Pruned" nodes.
    #[inline]
    pub fn key_range<Q1: ?Sized, Q2: ?Sized>(&self, first: &Q1, last: &Q2) -> HashTree<'_>
    where
        K: Borrow<Q1> + Borrow<Q2>,
        Q1: Ord,
        Q2: Ord,
    {
        self.range_witness(
            self.lower_bound(first),
            self.upper_bound(last),
            Node::witness_tree,
        )
    }

    /// Returns a witness for the key-value pairs in the specified range.
    /// The resulting tree contains both keys and values.
    #[inline]
    pub fn value_range<Q1: ?Sized, Q2: ?Sized>(&self, first: &Q1, last: &Q2) -> HashTree<'_>
    where
        K: Borrow<Q1> + Borrow<Q2>,
        Q1: Ord,
        Q2: Ord,
    {
        self.range_witness(
            self.lower_bound(first),
            self.upper_bound(last),
            Node::data_tree,
        )
    }

    /// Returns a witness that enumerates all the keys starting with
    /// the specified prefix.
    #[inline]
    pub fn keys_with_prefix<P: ?Sized>(&self, prefix: &P) -> HashTree<'_>
    where
        K: Prefix<P>,
        P: Ord,
    {
        self.range_witness(
            self.lower_bound(prefix),
            self.right_prefix_neighbor(prefix),
            Node::witness_tree,
        )
    }

    /// Enumerates all the key-value pairs in the tree.
    #[inline]
    pub fn for_each<'a, F>(&'a self, mut f: F)
    where
        F: 'a + FnMut(&'a K, &'a V),
    {
        unsafe fn visit<'a, K, V, F>(n: *mut Node<K, V>, f: &mut F)
        where
            F: 'a + FnMut(&'a K, &'a V),
            K: 'static + Label,
            V: 'a + AsHashTree,
        {
            debug_assert!(!n.is_null());
            if !(*n).left.is_null() {
                visit((*n).left, f)
            }
            (*f)(&(*n).key, &(*n).value);
            if !(*n).right.is_null() {
                visit((*n).right, f)
            }
        }
        if self.root.is_null() {
            return;
        }
        unsafe { visit(self.root, &mut f) }
    }

    fn witness_range_above<'a>(
        &'a self,
        lo: KeyBound<'a, K>,
        f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
    ) -> HashTree<'a> {
        unsafe fn go<'a, K: 'static + Label, V: AsHashTree + 'static>(
            n: *mut Node<K, V>,
            lo: KeyBound<'a, K>,
            f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
        ) -> HashTree<'a> {
            if n.is_null() {
                return Empty;
            }
            match (*n).key.cmp(lo.as_ref()) {
                Equal => three_way_fork(
                    Node::left_hash_tree(n),
                    match lo {
                        KeyBound::Exact(_) => f(n),
                        KeyBound::Neighbor(_) => Node::witness_tree(n),
                    },
                    Node::full_witness_tree((*n).right, f),
                ),
                Less => three_way_fork(
                    Node::left_hash_tree(n),
                    Pruned(Node::data_hash(n)),
                    go((*n).right, lo, f),
                ),
                Greater => three_way_fork(
                    go((*n).left, lo, f),
                    f(n),
                    Node::full_witness_tree((*n).right, f),
                ),
            }
        }
        unsafe { go(self.root, lo, f) }
    }

    fn witness_range_below<'a>(
        &'a self,
        hi: KeyBound<'a, K>,
        f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
    ) -> HashTree<'a> {
        unsafe fn go<'a, K: 'static + Label, V: AsHashTree + 'static>(
            n: *mut Node<K, V>,
            hi: KeyBound<'a, K>,
            f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
        ) -> HashTree<'a> {
            if n.is_null() {
                return Empty;
            }
            match (*n).key.cmp(hi.as_ref()) {
                Equal => three_way_fork(
                    Node::full_witness_tree((*n).left, f),
                    match hi {
                        KeyBound::Exact(_) => f(n),
                        KeyBound::Neighbor(_) => Node::witness_tree(n),
                    },
                    Node::right_hash_tree(n),
                ),
                Greater => three_way_fork(
                    go((*n).left, hi, f),
                    Pruned(Node::data_hash(n)),
                    Node::right_hash_tree(n),
                ),
                Less => three_way_fork(
                    Node::full_witness_tree((*n).left, f),
                    f(n),
                    go((*n).right, hi, f),
                ),
            }
        }
        unsafe { go(self.root, hi, f) }
    }

    fn witness_range_between<'a>(
        &'a self,
        lo: KeyBound<'a, K>,
        hi: KeyBound<'a, K>,
        f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
    ) -> HashTree<'a> {
        debug_assert!(
            lo.as_ref() <= hi.as_ref(),
            "lo = {:?} > hi = {:?}",
            lo.as_ref().as_label(),
            hi.as_ref().as_label()
        );
        unsafe fn go<'a, K: 'static + Label, V: AsHashTree + 'static>(
            n: *mut Node<K, V>,
            lo: KeyBound<'a, K>,
            hi: KeyBound<'a, K>,
            f: unsafe fn(*mut Node<K, V>) -> HashTree<'a>,
        ) -> HashTree<'a> {
            if n.is_null() {
                return Empty;
            }
            let k = &(*n).key;
            match (lo.as_ref().cmp(k), k.cmp(hi.as_ref())) {
                (Less, Less) => {
                    let left = go((*n).left, lo, hi, f);
                    let right = go((*n).right, lo, hi, f);
                    three_way_fork(left, f(n), right)
                }
                (Equal, Equal) => three_way_fork(
                    Node::left_hash_tree(n),
                    match (lo, hi) {
                        (KeyBound::Exact(_), _) => f(n),
                        (_, KeyBound::Exact(_)) => f(n),
                        _ => Node::witness_tree(n),
                    },
                    Node::right_hash_tree(n),
                ),
                (_, Equal) => three_way_fork(
                    go((*n).left, lo, hi, f),
                    match hi {
                        KeyBound::Exact(_) => f(n),
                        KeyBound::Neighbor(_) => Node::witness_tree(n),
                    },
                    Node::right_hash_tree(n),
                ),
                (Equal, _) => three_way_fork(
                    Node::left_hash_tree(n),
                    match lo {
                        KeyBound::Exact(_) => f(n),
                        KeyBound::Neighbor(_) => Node::witness_tree(n),
                    },
                    go((*n).right, lo, hi, f),
                ),
                (Less, Greater) => three_way_fork(
                    go((*n).left, lo, hi, f),
                    Pruned(Node::data_hash(n)),
                    Node::right_hash_tree(n),
                ),
                (Greater, Less) => three_way_fork(
                    Node::left_hash_tree(n),
                    Pruned(Node::data_hash(n)),
                    go((*n).right, lo, hi, f),
                ),
                _ => Pruned((*n).subtree_hash),
            }
        }
        unsafe { go(self.root, lo, hi, f) }
    }

    fn lower_bound<Q: ?Sized>(&self, key: &Q) -> Option<KeyBound<'_, K>>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V, Q: ?Sized>(
            n: *mut Node<K, V>,
            key: &Q,
        ) -> Option<KeyBound<'a, K>>
        where
            K: Borrow<Q>,
            Q: Ord,
        {
            if n.is_null() {
                return None;
            }
            let node_key = &(*n).key;
            match node_key.borrow().cmp(key) {
                Less => go((*n).right, key).or(Some(KeyBound::Neighbor(node_key))),
                Equal => Some(KeyBound::Exact(node_key)),
                Greater => go((*n).left, key),
            }
        }
        unsafe { go(self.root, key) }
    }

    fn upper_bound<Q: ?Sized>(&self, key: &Q) -> Option<KeyBound<'_, K>>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V, Q: ?Sized>(
            n: *mut Node<K, V>,
            key: &Q,
        ) -> Option<KeyBound<'a, K>>
        where
            K: Borrow<Q>,
            Q: Ord,
        {
            if n.is_null() {
                return None;
            }
            let node_key = &(*n).key;
            match node_key.borrow().cmp(key) {
                Less => go((*n).right, key),
                Equal => Some(KeyBound::Exact(node_key)),
                Greater => go((*n).left, key).or(Some(KeyBound::Neighbor(node_key))),
            }
        }
        unsafe { go(self.root, key) }
    }

    fn right_prefix_neighbor<P: ?Sized>(&self, prefix: &P) -> Option<KeyBound<'_, K>>
    where
        K: Prefix<P>,
        P: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V, P: ?Sized>(
            n: *mut Node<K, V>,
            prefix: &P,
        ) -> Option<KeyBound<'a, K>>
        where
            K: Prefix<P>,
            P: Ord,
        {
            if n.is_null() {
                return None;
            }
            let node_key = &(*n).key;
            let key_prefix = node_key.borrow();
            match key_prefix.cmp(prefix) {
                Greater if node_key.is_prefix(prefix) => go((*n).right, prefix),
                Greater => go((*n).left, prefix).or(Some(KeyBound::Neighbor(node_key))),
                Less | Equal => go((*n).right, prefix),
            }
        }
        unsafe { go(self.root, prefix) }
    }

    fn lookup_and_build_witness<'a, Q: ?Sized>(
        &'a self,
        key: &Q,
        f: impl FnOnce(&'a V) -> HashTree<'a>,
    ) -> Option<HashTree<'a>>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe fn go<'a, K: 'static + Label, V: AsHashTree + 'static, Q: ?Sized>(
            n: *mut Node<K, V>,
            key: &Q,
            f: impl FnOnce(&'a V) -> HashTree<'a>,
        ) -> Option<HashTree<'a>>
        where
            K: Borrow<Q>,
            Q: Ord,
        {
            if n.is_null() {
                return None;
            }
            match key.cmp((*n).key.borrow()) {
                Equal => Some(three_way_fork(
                    Node::left_hash_tree(n),
                    Node::subtree_with(n, f),
                    Node::right_hash_tree(n),
                )),
                Less => {
                    let subtree = go((*n).left, key, f)?;
                    Some(three_way_fork(
                        subtree,
                        Pruned(Node::data_hash(n)),
                        Node::right_hash_tree(n),
                    ))
                }
                Greater => {
                    let subtree = go((*n).right, key, f)?;
                    Some(three_way_fork(
                        Node::left_hash_tree(n),
                        Pruned(Node::data_hash(n)),
                        subtree,
                    ))
                }
            }
        }
        unsafe { go(self.root, key, f) }
    }

    /// Inserts a key-value entry into the map.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> (Option<V>, &mut V) {
        struct GoResult<'a, K, V> {
            node: *mut Node<K, V>,
            old_value: Option<V>,
            new_value_ref: &'a mut V,
        }

        unsafe fn go<K: 'static + Label, V: AsHashTree + 'static>(
            mut h: *mut Node<K, V>,
            k: K,
            mut v: V,
        ) -> GoResult<'static, K, V> {
            if h.is_null() {
                let node = Node::new(k, v);
                return GoResult {
                    node,
                    old_value: None,
                    new_value_ref: &mut (*node).value,
                };
            }

            let (old_value, new_value_ref) = match k.cmp(&(*h).key) {
                Equal => {
                    std::mem::swap(&mut (*h).value, &mut v);
                    (*h).subtree_hash = Node::subtree_hash(h);
                    (Some(v), &mut (*h).value)
                }
                Less => {
                    let res = go((*h).left, k, v);
                    (*h).left = res.node;
                    (*h).subtree_hash = Node::subtree_hash(h);
                    (res.old_value, res.new_value_ref)
                }
                Greater => {
                    let res = go((*h).right, k, v);
                    (*h).right = res.node;
                    (*h).subtree_hash = Node::subtree_hash(h);
                    (res.old_value, res.new_value_ref)
                }
            };

            GoResult {
                node: balance(h),
                old_value,
                new_value_ref,
            }
        }

        unsafe {
            let mut result = go(self.root, key, value);
            (*result.node).color = Color::Black;

            #[cfg(test)]
            debug_assert!(
                is_balanced(result.node),
                "the tree is not balanced:\n{:?}",
                DebugView(result.node)
            );
            #[cfg(test)]
            debug_assert!(!has_dangling_pointers(result.node));

            if result.old_value.is_none() {
                self.len += 1;
            }

            self.root = result.node;
            (result.old_value, result.new_value_ref)
        }
    }

    /// Removes the specified key from the map.
    #[inline]
    pub fn delete<Q: ?Sized>(&mut self, key: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Ord,
    {
        unsafe fn move_red_left<K: 'static + Label, V: AsHashTree + 'static>(
            mut h: *mut Node<K, V>,
        ) -> *mut Node<K, V> {
            flip_colors(h);
            if is_red((*(*h).right).left) {
                (*h).right = rotate_right((*h).right);
                h = rotate_left(h);
                flip_colors(h);
            }
            h
        }

        unsafe fn move_red_right<K: 'static + Label, V: AsHashTree + 'static>(
            mut h: *mut Node<K, V>,
        ) -> *mut Node<K, V> {
            flip_colors(h);
            if is_red((*(*h).left).left) {
                h = rotate_right(h);
                flip_colors(h);
            }
            h
        }

        #[inline]
        unsafe fn min<K: 'static + Label, V: AsHashTree + 'static>(
            mut h: *mut Node<K, V>,
        ) -> *mut Node<K, V> {
            while !(*h).left.is_null() {
                h = (*h).left;
            }
            h
        }

        unsafe fn delete_min<K: 'static + Label, V: AsHashTree + 'static>(
            mut h: *mut Node<K, V>,
            result: &mut Option<(K, V)>,
        ) -> *mut Node<K, V> {
            if (*h).left.is_null() {
                debug_assert!((*h).right.is_null());
                *result = Some(Node::delete(h).unwrap());
                return Node::null();
            }
            if !is_red((*h).left) && !is_red((*(*h).left).left) {
                h = move_red_left(h);
            }
            (*h).left = delete_min((*h).left, result);
            (*h).subtree_hash = Node::subtree_hash(h);
            balance(h)
        }

        unsafe fn go<K: 'static + Label, V: AsHashTree + 'static, Q: ?Sized>(
            mut h: *mut Node<K, V>,
            result: &mut Option<(K, V)>,
            key: &Q,
        ) -> *mut Node<K, V>
        where
            K: Borrow<Q>,
            Q: Ord,
        {
            if key < (*h).key.borrow() {
                if !is_red((*h).left) && !is_red((*(*h).left).left) {
                    h = move_red_left(h);
                }
                (*h).left = go((*h).left, result, key);
            } else {
                if is_red((*h).left) {
                    h = rotate_right(h);
                }
                if key == (*h).key.borrow() && (*h).right.is_null() {
                    debug_assert!((*h).left.is_null());
                    *result = Some(Node::delete(h).unwrap());
                    return Node::null();
                }

                if !is_red((*h).right) && !is_red((*(*h).right).left) {
                    h = move_red_right(h);
                }

                if key == (*h).key.borrow() {
                    let m = min((*h).right);
                    std::mem::swap(&mut (*h).key, &mut (*m).key);
                    std::mem::swap(&mut (*h).value, &mut (*m).value);
                    (*h).right = delete_min((*h).right, result);
                } else {
                    (*h).right = go((*h).right, result, key);
                }
            }
            (*h).subtree_hash = Node::subtree_hash(h);
            balance(h)
        }

        unsafe {
            self.get(key)?;
            if !is_red((*self.root).left) && !is_red((*self.root).right) {
                (*self.root).color = Color::Red;
            }

            let mut result = None;
            self.root = go(self.root, &mut result, key);
            if !self.root.is_null() {
                (*self.root).color = Color::Black;
            }

            #[cfg(test)]
            debug_assert!(
                is_balanced(self.root),
                "unbalanced map: {:?}",
                DebugView(self.root)
            );

            #[cfg(test)]
            debug_assert!(result.is_some());
            self.len -= 1;

            debug_assert!(self.get(key).is_none());
            result
        }
    }
}

fn three_way_fork<'a>(l: HashTree<'a>, m: HashTree<'a>, r: HashTree<'a>) -> HashTree<'a> {
    match (l, m, r) {
        (Empty, m, Empty) => m,
        (l, m, Empty) => fork(l, m),
        (Empty, m, r) => fork(m, r),
        (Pruned(lhash), Pruned(mhash), Pruned(rhash)) => {
            Pruned(fork_hash(&lhash, &fork_hash(&mhash, &rhash)))
        }
        (l, Pruned(mhash), Pruned(rhash)) => fork(l, Pruned(fork_hash(&mhash, &rhash))),
        (l, m, r) => fork(l, fork(m, r)),
    }
}

// helper functions
unsafe fn is_red<K, V>(x: *const Node<K, V>) -> bool {
    if x.is_null() {
        false
    } else {
        (*x).color == Color::Red
    }
}

unsafe fn balance<K: Label + 'static, V: AsHashTree + 'static>(
    mut h: *mut Node<K, V>,
) -> *mut Node<K, V> {
    assert!(!h.is_null());

    if is_red((*h).right) && !is_red((*h).left) {
        h = rotate_left(h);
    }
    if is_red((*h).left) && is_red((*(*h).left).left) {
        h = rotate_right(h);
    }
    if is_red((*h).left) && is_red((*h).right) {
        flip_colors(h)
    }
    h
}

/// Make a left-leaning link lean to the right.
unsafe fn rotate_right<K: 'static + Label, V: AsHashTree + 'static>(
    h: *mut Node<K, V>,
) -> *mut Node<K, V> {
    debug_assert!(!h.is_null());
    debug_assert!(is_red((*h).left));

    let mut x = (*h).left;
    (*h).left = (*x).right;
    (*x).right = h;
    (*x).color = (*(*x).right).color;
    (*(*x).right).color = Color::Red;

    (*h).subtree_hash = Node::subtree_hash(h);
    (*x).subtree_hash = Node::subtree_hash(x);

    x
}

unsafe fn rotate_left<K: 'static + Label, V: AsHashTree + 'static>(
    h: *mut Node<K, V>,
) -> *mut Node<K, V> {
    debug_assert!(!h.is_null());
    debug_assert!(is_red((*h).right));

    let mut x = (*h).right;
    (*h).right = (*x).left;
    (*x).left = h;
    (*x).color = (*(*x).left).color;
    (*(*x).left).color = Color::Red;

    (*h).subtree_hash = Node::subtree_hash(h);
    (*x).subtree_hash = Node::subtree_hash(x);

    x
}

unsafe fn flip_colors<K, V>(h: *mut Node<K, V>) {
    (*h).color = (*h).color.flip();
    (*(*h).left).color = (*(*h).left).color.flip();
    (*(*h).right).color = (*(*h).right).color.flip();
}

#[cfg(test)]
unsafe fn is_balanced<K, V>(root: *mut Node<K, V>) -> bool {
    unsafe fn go<K, V>(node: *mut Node<K, V>, mut num_black: usize) -> bool {
        if node.is_null() {
            return num_black == 0;
        }
        if !is_red(node) {
            debug_assert!(num_black > 0);
            num_black -= 1;
        } else {
            assert!(!is_red((*node).left));
            assert!(!is_red((*node).right));
        }
        go((*node).left, num_black) && go((*node).right, num_black)
    }

    let mut num_black = 0;
    let mut x = root;
    while !x.is_null() {
        if !is_red(x) {
            num_black += 1;
        }
        x = (*x).left;
    }
    go(root, num_black)
}

#[cfg(test)]
unsafe fn has_dangling_pointers<K, V>(root: *mut Node<K, V>) -> bool {
    if root.is_null() {
        return false;
    }

    !debug_alloc::is_live(root)
        || has_dangling_pointers((*root).left)
        || has_dangling_pointers((*root).right)
}

struct DebugView<K, V>(*const Node<K, V>);

impl<K: Label, V> fmt::Debug for DebugView<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe fn go<K: Label, V>(
            f: &mut fmt::Formatter<'_>,
            h: *const Node<K, V>,
            offset: usize,
        ) -> fmt::Result {
            if h.is_null() {
                writeln!(f, "{:width$}[B] <null>", "", width = offset)
            } else {
                writeln!(
                    f,
                    "{:width$}[{}] {:?}",
                    "",
                    if is_red(h) { "R" } else { "B" },
                    (*h).key.as_label(),
                    width = offset
                )?;
                go(f, (*h).left, offset + 2)?;
                go(f, (*h).right, offset + 2)
            }
        }
        unsafe { go(f, self.0, 0) }
    }
}

#[cfg(test)]
mod test;
