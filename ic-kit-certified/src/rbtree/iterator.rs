use super::{Node, RbTree};
use crate::label::Label;
use crate::AsHashTree;
use std::marker::PhantomData;

/// An iterator over key-values in a RbTree.
pub struct RbTreeIterator<'tree, K: 'static + Label, V: AsHashTree + 'static> {
    visit: *mut Node<K, V>,
    stack: Vec<*mut Node<K, V>>,
    remaining_elements: usize,
    lifetime: PhantomData<&'tree RbTree<K, V>>,
}

impl<'tree, K: 'static + Label, V: AsHashTree + 'static> RbTreeIterator<'tree, K, V> {
    pub fn new(tree: &'tree RbTree<K, V>) -> Self {
        Self {
            visit: tree.root,
            stack: Vec::with_capacity(8),
            remaining_elements: tree.len(),
            lifetime: PhantomData::default(),
        }
    }
}

impl<'tree, K: 'static + Label, V: AsHashTree + 'static> Iterator for RbTreeIterator<'tree, K, V> {
    type Item = (&'tree K, &'tree V);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            while !self.visit.is_null() {
                self.stack.push(self.visit);
                self.visit = (*self.visit).left;
            }

            if let Some(node) = self.stack.pop() {
                self.visit = (*node).right;
                self.remaining_elements -= 1;
                return Some((&(*node).key, &(*node).value));
            }

            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining_elements, Some(self.remaining_elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_visit_all() {
        let mut tree = RbTree::<[u8; 1], u8>::new();

        for i in 0..250u8 {
            tree.insert([i], i);
        }

        let iter = RbTreeIterator::new(&tree);

        let mut expected_v = 0u8;

        for (_, v) in iter {
            assert_eq!(v, &expected_v);
            expected_v += 1;
        }

        assert_eq!(expected_v, 250);
    }
}
