use super::{Node, RbTree};
use crate::label::Label;
use crate::AsHashTree;
use std::fmt::{self, Debug};
use Entry::{Occupied, Vacant};

/// A view into a single entry in a map, which may either be vacant or occupied.
///
/// This `enum` is constructed from the [`entry`] method on [`RbTree`].
///
/// [`entry`]: RbTree::entry
pub enum Entry<'a, K: 'static + Label, V: AsHashTree + 'static> {
    Vacant(VacantEntry<'a, K, V>),
    Occupied(OccupiedEntry<'a, K, V>),
}

/// A view into a vacant entry in a [`RbTree`]. It is part of the [`Entry`] enum.
pub struct VacantEntry<'a, K: 'static + Label, V: AsHashTree + 'static> {
    pub(super) map: &'a mut RbTree<K, V>,
    pub(super) key: K,
}

pub struct OccupiedEntry<'a, K: 'static + Label, V: AsHashTree + 'static> {
    pub(super) map: &'a mut RbTree<K, V>,
    pub(super) key: K,
    pub(super) node: *mut Node<K, V>,
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> VacantEntry<'a, K, V> {
    /// Sets the value of the entry with the VacantEntry’s key, and returns a mutable
    /// reference to it.
    #[inline]
    pub fn insert(self, value: V) -> &'a mut V {
        self.map.insert(self.key, value).1
    }

    /// Take ownership of the key.
    #[inline]
    pub fn into_key(self) -> K {
        self.key
    }

    /// Gets a reference to the key that would be used when inserting a value through
    /// the [`VacantEntry`].
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> OccupiedEntry<'a, K, V> {
    /// Gets a reference to the value in the entry.
    #[inline]
    pub fn get(&self) -> &V {
        unsafe { &(*self.node).value }
    }

    /// Gets a mutable reference to the value in the entry.
    ///
    /// If you need a reference to the `OccupiedEntry` that may outlive the destruction of
    /// the `Entry` value, see [`into_mut`].
    ///
    /// [`into_mut`]: OccupiedEntry::into_mut
    #[inline]
    pub fn get_mut(&mut self) -> &mut V {
        unsafe { &mut (*self.node).value }
    }

    /// Converts the entry into a mutable reference to its value.
    ///
    /// If you need multiple references to the OccupiedEntry, see [`get_mut`].
    ///
    /// [`get_mut`]: OccupiedEntry::get_mut
    #[inline]
    pub fn into_mut(self) -> &'a mut V {
        unsafe { &mut (*self.node).value }
    }

    /// Gets a reference to the key in the entry.
    #[inline]
    pub fn key(&self) -> &K {
        &self.key
    }

    /// Takes the value of the entry out of the map, and returns it.
    #[inline]
    pub fn remove(self) -> V {
        self.map.delete(&self.key).unwrap().1
    }

    /// Take ownership of the key and value from the map.
    #[inline]
    pub fn remove_entry(self) -> (K, V) {
        self.map.delete(&self.key).unwrap()
    }
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> Entry<'a, K, V> {
    /// Provides in-place mutable access to an occupied entry before any
    /// potential inserts into the map.
    #[inline]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Occupied(mut entry) => {
                f(entry.get_mut());
                Occupied(entry)
            }
            Vacant(entry) => Vacant(entry),
        }
    }

    /// Ensures a value is in the entry by inserting the default value if empty,
    /// and returns a mutable reference to the value in the entry.
    #[inline]
    pub fn or_default(self) -> &'a mut V
    where
        V: Default,
    {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(Default::default()),
        }
    }

    /// Ensures a value is in the entry by inserting the default if empty, and returns
    /// a mutable reference to the value in the entry.
    #[inline]
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(default),
        }
    }

    /// Ensures a value is in the entry by inserting the result of the default function if empty,
    /// and returns a mutable reference to the value in the entry.
    #[inline]
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(default()),
        }
    }

    /// Ensures a value is in the entry by inserting, if empty, the result of the default function.
    /// This method allows for generating key-derived values for insertion by providing the default
    /// function a reference to the key that was moved during the `.entry(key)` method call.
    ///
    /// The reference to the moved key is provided so that cloning or copying the key is
    /// unnecessary, unlike with `.or_insert_with(|| ... )`.
    #[inline]
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// Returns a reference to this entry’s key.
    #[inline]
    pub fn key(&self) -> &K {
        match self {
            Occupied(entry) => entry.key(),
            Vacant(entry) => entry.key(),
        }
    }
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> Debug for Entry<'a, K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> Debug for VacantEntry<'a, K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

impl<'a, K: 'static + Label, V: AsHashTree + 'static> Debug for OccupiedEntry<'a, K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish()
    }
}
