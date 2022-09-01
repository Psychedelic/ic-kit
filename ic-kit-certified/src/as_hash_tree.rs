use crate::hashtree::leaf_hash;
use crate::{Hash, HashTree};
use candid::{Nat, Principal};
use std::borrow::Cow;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

/// Defines any type that can be converted to a [`HashTree`].
///
/// For number types this trait is implemented to use the BigEndian byte ordering to represent
/// the number as a [`[u8]`].
pub trait AsHashTree {
    /// This method should return the root hash of this hash tree.
    /// Must be equivalent to `as_hash_tree().reconstruct()`.
    ///
    /// Only change the default implementation if you have a better implementation
    /// that does not rely on reconstructing a tree.
    fn root_hash(&self) -> Hash {
        self.as_hash_tree().reconstruct()
    }

    /// Constructs a hash tree corresponding to the data.
    fn as_hash_tree(&self) -> HashTree<'_>;
}

impl AsHashTree for Vec<u8> {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(self.as_slice())
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::from(self.as_slice()))
    }
}

impl AsHashTree for &[u8] {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(self)
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::from(*self))
    }
}

impl AsHashTree for bool {
    #[inline]
    fn root_hash(&self) -> Hash {
        if *self {
            leaf_hash(&[1])
        } else {
            leaf_hash(&[0])
        }
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        let value = if *self { vec![1] } else { vec![0] };
        HashTree::Leaf(Cow::Owned(value))
    }
}

impl AsHashTree for String {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(self.as_bytes())
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::from(self.as_bytes()))
    }
}

impl AsHashTree for &str {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(self.as_bytes())
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::from(self.as_bytes()))
    }
}

impl AsHashTree for Principal {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(self.as_slice())
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::from(self.as_slice()))
    }
}

impl AsHashTree for Nat {
    #[inline]
    fn root_hash(&self) -> Hash {
        leaf_hash(&self.0.to_bytes_be())
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        HashTree::Leaf(Cow::Owned(self.0.to_bytes_be()))
    }
}

impl<T> AsHashTree for Box<T>
where
    T: AsHashTree,
{
    #[inline]
    fn root_hash(&self) -> Hash {
        self.as_ref().root_hash()
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        self.as_ref().as_hash_tree()
    }
}

impl<T> AsHashTree for Rc<T>
where
    T: AsHashTree,
{
    #[inline]
    fn root_hash(&self) -> Hash {
        self.as_ref().root_hash()
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        self.as_ref().as_hash_tree()
    }
}

impl<T> AsHashTree for Arc<T>
where
    T: AsHashTree,
{
    #[inline]
    fn root_hash(&self) -> Hash {
        self.as_ref().root_hash()
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        self.as_ref().as_hash_tree()
    }
}

impl<T> AsHashTree for NonNull<T>
where
    T: AsHashTree,
{
    #[inline]
    fn root_hash(&self) -> Hash {
        unsafe { self.as_ref().root_hash() }
    }

    #[inline]
    fn as_hash_tree(&self) -> HashTree<'_> {
        unsafe { self.as_ref().as_hash_tree() }
    }
}

macro_rules! impl_fixed_size {
    ( $($size:expr),* ) => {
        $(
            impl AsHashTree for [u8; $size] {
                #[inline]
                fn root_hash(&self) -> Hash {
                    leaf_hash(self)
                }

                #[inline]
                fn as_hash_tree(&self) -> HashTree<'_> {
                    HashTree::Leaf(Cow::from(self as &[u8]))
                }
            }
        )*
    }
}

macro_rules! impl_num {
    ( $($name:ty),* ) => {
        $(
            impl AsHashTree for $name {
                #[inline]
                fn root_hash(&self) -> Hash {
                    leaf_hash(&self.to_be_bytes())
                }

                #[inline]
                fn as_hash_tree(&self) -> HashTree<'_> {
                    let bytes = self.to_be_bytes();
                    HashTree::Leaf(Cow::Owned(bytes.into()))
                }
            }
        )*
    }
}

impl_num!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, usize, isize, f32, f64);
impl_fixed_size!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32
);
