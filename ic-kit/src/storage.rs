#![allow(non_snake_case)]
use std::any::{Any, TypeId};
use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::ops::DerefMut;

type StorageMap = HashMap<TypeId, RefCell<Box<dyn Any>>>;

/// An storage implementation for singleton design pattern, where we only have one value
/// associated with each types.
#[derive(Default)]
pub struct Storage {
    storage: RefCell<StorageMap>,
}

impl Storage {
    /// Ensure the default value exists on the map.
    #[inline(always)]
    fn ensure_default<T: 'static + Default>(&self, tid: TypeId) {
        self.storage
            .borrow_mut()
            .entry(tid)
            .or_insert_with(|| RefCell::new(Box::new(T::default())));
    }

    /// Pass an immutable reference to the stored data of the type `T` to the closure,
    /// if there is no data associated with the type, store the `Default` and then perform the
    /// operation.
    #[inline]
    pub fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(&self, callback: F) -> U {
        let tid = TypeId::of::<T>();
        self.ensure_default::<T>(tid);
        let cell = unsafe { self.storage.try_borrow_unguarded() }
            .unwrap()
            .get(&tid)
            .unwrap()
            .borrow();
        let borrow = cell.downcast_ref::<T>().unwrap();
        callback(borrow)
    }

    /// Pass an immutable reference to the stored data of the type `T` to the closure,
    /// if there is no data associated with the type, just return None.
    #[inline]
    pub fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(&self, callback: F) -> Option<U> {
        let tid = TypeId::of::<T>();
        unsafe { self.storage.try_borrow_unguarded() }
            .unwrap()
            .get(&tid)
            .map(|c| c.borrow())
            .map(|c| callback(c.borrow().downcast_ref::<T>().unwrap()))
    }

    /// Like [`Self::with`] but passes a mutable reference.
    #[inline]
    pub fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> U {
        let tid = TypeId::of::<T>();
        self.ensure_default::<T>(tid);
        let mut cell = unsafe { self.storage.try_borrow_unguarded() }
            .unwrap()
            .get(&tid)
            .unwrap()
            .borrow_mut();
        let borrow = cell.downcast_mut::<T>().unwrap();
        callback(borrow)
    }

    /// Like [`Self::maybe_with`] but passes a mutable reference.
    #[inline]
    pub fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> Option<U> {
        let tid = TypeId::of::<T>();
        unsafe { self.storage.try_borrow_unguarded() }
            .unwrap()
            .get(&tid)
            .map(|c| c.borrow_mut())
            .map(|mut c| callback(c.borrow_mut().downcast_mut::<T>().unwrap()))
    }

    /// Remove the data associated with the type `T`, and returns it if any.
    #[inline]
    pub fn take<T: 'static>(&self) -> Option<T> {
        let tid = TypeId::of::<T>();
        self.storage
            .borrow_mut()
            .remove(&tid)
            .map(|cell| *cell.into_inner().downcast::<T>().unwrap())
    }

    /// Store the given value for type `T`, returns the previously stored value if any.
    #[inline]
    pub fn swap<T: 'static>(&self, value: T) -> Option<T> {
        let tid = TypeId::of::<T>();
        match self.storage.borrow_mut().entry(tid) {
            Entry::Occupied(mut o) => Some(
                *o.get_mut()
                    .replace(Box::new(value))
                    .downcast::<T>()
                    .unwrap(),
            ),
            Entry::Vacant(v) => {
                v.insert(RefCell::new(Box::new(value)));
                None
            }
        }
    }

    /// Just like `.with` but can pass the immutable reference to many items in one closure.
    #[inline]
    pub fn with_many<A: BorrowMany, U, F: FnOnce(A) -> U>(&self, callback: F) -> U {
        {
            A::ensure_default(self.storage.borrow_mut().deref_mut())
        }

        let storage = unsafe { self.storage.try_borrow_unguarded() }.unwrap();
        A::with(storage, callback)
    }

    /// Just like `.with_mut` but can pass the mutable reference to many items in one closure.
    #[inline]
    pub fn with_many_mut<A: BorrowMutMany, U, F: FnOnce(A) -> U>(&self, callback: F) -> U {
        {
            A::ensure_default(self.storage.borrow_mut().deref_mut())
        }

        let storage = unsafe { self.storage.try_borrow_unguarded() }.unwrap();
        A::with_mut(storage, callback)
    }
}

pub trait BorrowMany: Sized {
    fn ensure_default(storage: &mut StorageMap);

    fn with<U, F: FnOnce(Self) -> U>(storage: &StorageMap, callback: F) -> U;
}

pub trait BorrowMutMany: Sized {
    fn ensure_default(storage: &mut StorageMap);

    fn with_mut<U, F: FnOnce(Self) -> U>(storage: &StorageMap, callback: F) -> U;
}

macro_rules! implement_borrow_many {
    ($(($($name: ident)+))+) => {
        $(
        impl<'a, $($name: 'static,)+> BorrowMany for ($(&'a $name,)+)
        where
            $($name: Default,)+
        {
            #[inline(always)]
            fn ensure_default(storage: &mut StorageMap) {
                $(
                storage
                    .entry(TypeId::of::<$name>())
                    .or_insert_with(|| RefCell::new(Box::new($name::default())));
                )+
            }

            #[inline(always)]
            fn with<U, F: FnOnce(($(&'a $name,)+)) -> U>(storage: &StorageMap, callback: F) -> U {
                $(
                let $name = storage.get(&TypeId::of::<$name>()).unwrap().borrow();
                )+

                callback((
                    $(
                    unsafe { &*(&*$name as *const Box<dyn Any>) }
                        .downcast_ref::<$name>()
                        .unwrap(),
                    )+
                ))
            }
        }

        impl<'a, $($name: 'static,)+> BorrowMutMany for ($(&'a mut $name,)+)
        where
            $($name: Default,)+
        {
            #[inline(always)]
            fn ensure_default(storage: &mut StorageMap) {
                $(
                storage
                    .entry(TypeId::of::<$name>())
                    .or_insert_with(|| RefCell::new(Box::new($name::default())));
                )+
            }

            #[inline(always)]
            fn with_mut<U, F: FnOnce(($(&'a mut $name,)+)) -> U>(storage: &StorageMap, callback: F) -> U {
                $(
                let mut $name = storage.get(&TypeId::of::<$name>()).unwrap().borrow_mut();
                )+

                callback((
                    $(
                    unsafe { &mut *(&mut *$name as *mut Box<dyn Any>) }
                        .downcast_mut::<$name>()
                        .unwrap(),
                    )+
                ))
            }
        }
        )+
    };
}

implement_borrow_many!(
    (A0)
    (A0 A1)
    (A0 A1 A3)
    (A0 A1 A3 A4)
    (A0 A1 A3 A4 A5)
    (A0 A1 A3 A4 A5 A6)
    (A0 A1 A3 A4 A5 A6 A7)
    (A0 A1 A3 A4 A5 A6 A7 A8)
    (A0 A1 A3 A4 A5 A6 A7 A8 A9)
);
