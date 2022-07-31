use std::any::{Any, TypeId};
use std::collections::HashMap;

/// An storage implementation for singleton design pattern, where we only have one value
/// associated with each types.
#[derive(Default)]
pub struct Storage {
    // TODO(qti3e) put Box in a RefCell when we get rid of get_mut::
    storage: HashMap<TypeId, Box<dyn Any>>,
}

impl Storage {
    /// Pass an immutable reference to the stored data of the type `T` to the closure,
    /// if there is no data associated with the type, store the `Default` and then perform the
    /// operation.
    #[inline]
    pub fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(&mut self, callback: F) -> U {
        let tid = TypeId::of::<T>();
        let cell = &*self
            .storage
            .entry(tid)
            .or_insert_with(|| Box::new(T::default()));
        let borrow = cell.downcast_ref::<T>().unwrap();
        callback(borrow)
    }

    /// Pass an immutable reference to the stored data of the type `T` to the closure,
    /// if there is no data associated with the type, just return None.
    #[inline]
    pub fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(&mut self, callback: F) -> Option<U> {
        let tid = TypeId::of::<T>();
        self.storage
            .get(&tid)
            .map(|cell| cell.downcast_ref::<T>().unwrap())
            .map(callback)
    }

    /// Like [`Self::with`] but passes a mutable reference.
    #[inline]
    pub fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(&mut self, callback: F) -> U {
        let tid = TypeId::of::<T>();
        let cell = self
            .storage
            .entry(tid)
            .or_insert_with(|| Box::new(T::default()));
        let borrow = cell.downcast_mut::<T>().unwrap();
        callback(borrow)
    }

    /// Like [`Self::maybe_with`] but passes a mutable reference.
    #[inline]
    pub fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(
        &mut self,
        callback: F,
    ) -> Option<U> {
        let tid = TypeId::of::<T>();
        self.storage
            .get_mut(&tid)
            .map(|cell| cell.downcast_mut::<T>().unwrap())
            .map(callback)
    }

    /// Remove the data associated with the type `T`, and returns it if any.
    #[inline]
    pub fn take<T: 'static>(&mut self) -> Option<T> {
        let tid = TypeId::of::<T>();
        self.storage
            .remove(&tid)
            .map(|cell| *cell.downcast::<T>().unwrap())
    }

    /// Store the given value for type `T`, returns the previously stored value if any.
    #[inline]
    pub fn swap<T: 'static>(&mut self, value: T) -> Option<T> {
        let tid = TypeId::of::<T>();
        self.storage
            .insert(tid, Box::new(value))
            .map(|cell| *cell.downcast::<T>().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::storage::Storage;

    #[derive(Default)]
    struct Counter {
        count: u64,
    }

    impl Counter {
        pub fn get(&self) -> u64 {
            self.count
        }

        pub fn increment(&mut self) -> u64 {
            self.count += 1;
            self.count
        }
    }

    #[test]
    fn test_storage() {
        let mut storage = Storage::default();
        assert_eq!(storage.with(Counter::get), 0);
        assert_eq!(storage.with_mut(Counter::increment), 1);
    }
}
