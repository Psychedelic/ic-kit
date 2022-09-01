use candid::Principal;
use std::borrow::{Borrow, Cow};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;

/// Any value that can be used as a label in the [`HashTree`] and can be a key
/// in the [`RbTree`].
///
/// [`HashTree`]: crate::HashTree
/// [`RbTree`]: crate::rbtree::RbTree
pub trait Label: Ord {
    fn as_label(&self) -> Cow<[u8]>;
}

/// A type `T` can be defined as prefix of type `U`, if they follow the same
/// representation and any valid value of `T` is also a valid head for a value
/// of type `U`.
///
/// The implementation should guarantee that the ordering is preserved which
/// implies:  
/// For any `u: U = x . y` where `x: T`:  
/// 1. `x0 < x1 => u0 < u1`  
/// 2. `x0 > x1 => u0 > u1`  
/// 3. `u0 == u1 => x0 == x1`
///
/// To implement this type, the Self (i.e `U`) should be borrowable as a `T`.
pub trait Prefix<T: Ord + ?Sized>: Label + Borrow<T> {
    /// Check if the provided value is the prefix of self. The default
    /// implementation only extracts the prefix from Self and checks
    /// for their equality which might not be true for some cases
    /// where we're dealing with slices of variable length for example.
    fn is_prefix(&self, prefix: &T) -> bool {
        self.borrow() == prefix
    }
}

impl Label for Vec<u8> {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self)
    }
}

impl Prefix<[u8]> for Vec<u8> {
    fn is_prefix(&self, prefix: &[u8]) -> bool {
        self.starts_with(prefix)
    }
}

impl Label for Box<[u8]> {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self)
    }
}

impl Prefix<[u8]> for Box<[u8]> {
    fn is_prefix(&self, prefix: &[u8]) -> bool {
        self.starts_with(prefix)
    }
}

impl Label for Principal {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_slice())
    }
}

impl Label for String {
    fn as_label(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}

impl Prefix<str> for String {
    fn is_prefix(&self, prefix: &str) -> bool {
        self.as_bytes().starts_with(prefix.as_bytes())
    }
}

impl Label for bool {
    fn as_label(&self) -> Cow<[u8]> {
        if *self {
            Cow::Owned(vec![1])
        } else {
            Cow::Owned(vec![0])
        }
    }
}

impl<T> Label for Box<T>
where
    T: Label,
{
    #[inline]
    fn as_label(&self) -> Cow<[u8]> {
        self.as_ref().as_label()
    }
}

impl<T> Label for Rc<T>
where
    T: Label,
{
    #[inline]
    fn as_label(&self) -> Cow<[u8]> {
        self.as_ref().as_label()
    }
}

impl<T> Label for Arc<T>
where
    T: Label,
{
    #[inline]
    fn as_label(&self) -> Cow<[u8]> {
        self.as_ref().as_label()
    }
}

impl<T> Label for NonNull<T>
where
    T: Label,
{
    #[inline]
    fn as_label(&self) -> Cow<[u8]> {
        unsafe { self.as_ref().as_label() }
    }
}

macro_rules! impl_fixed_size {
    ( $($size:expr),* ) => {
        $(
            impl Label for [u8; $size] {
                #[inline]
                fn as_label(&self) -> Cow<[u8]> {
                    Cow::Borrowed(self)
                }
            }

            impl Prefix<[u8]> for [u8; $size] {
                #[inline]
                fn is_prefix(&self, prefix: &[u8]) -> bool {
                    self.starts_with(prefix)
                }
            }
        )*
    }
}

impl_fixed_size!(
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32
);

macro_rules! impl_num {
    ( $($name:ty),* ) => {
        $(
            impl Label for $name {
                fn as_label(&self) -> Cow<[u8]> {
                    Cow::Owned(self.to_be_bytes().into())
                }
            }
        )*
    }
}

impl_num!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, usize, isize);
