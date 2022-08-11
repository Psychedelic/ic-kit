use crate::storage::{BorrowMany, BorrowMutMany, Storage};

thread_local! {
    static STORAGE: Storage = Storage::default();
}

/// Pass an immutable reference to the value associated with the given type to the closure.
///
/// If no value is currently associated to the type `T`, this method will insert the default
/// value in its place before invoking the callback. Use `maybe_with` if you don't want the
/// default value to be inserted or if your type does not implement the [`Default`] trait.
///
/// This is a safe replacement for the previously known `ic_kit::ic::get` API, and you can use it
/// instead of `lazy_static` or `local_thread`.
pub fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(callback: F) -> U {
    STORAGE.with(|storage| storage.with(callback))
}

/// Like [`with`], but does not initialize the data with the default value and simply returns None,
/// if there is no value associated with the type.
pub fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(callback: F) -> Option<U> {
    STORAGE.with(|storage| storage.maybe_with(callback))
}

/// Pass a mutable reference to the value associated with the given type to the closure.
///
/// If no value is currently associated to the type `T`, this method will insert the default
/// value in its place before invoking the callback. Use `maybe_with_mut` if you don't want the
/// default value to be inserted or if your type does not implement the [`Default`] trait.
///
/// This is a safe replacement for the previously known `ic_kit::ic::get` API, and you can use it
/// instead of `lazy_static` or `local_thread`.
pub fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(callback: F) -> U {
    STORAGE.with(|storage| storage.with_mut(callback))
}

/// Like [`with_mut`], but does not initialize the data with the default value and simply returns
/// None, if there is no value associated with the type.
pub fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(callback: F) -> Option<U> {
    STORAGE.with(|storage| storage.maybe_with_mut(callback))
}

/// Remove the current value associated with the type and return it.
pub fn take<T: 'static>() -> Option<T> {
    STORAGE.with(|storage| storage.take::<T>())
}

/// Swaps the value associated with type `T` with the given value, returns the old one.
pub fn swap<T: 'static>(value: T) -> Option<T> {
    STORAGE.with(|storage| storage.swap(value))
}

/// Like [`crate::ic::with`] but passes the immutable reference of multiple variables to the
/// closure as a tuple.
///
/// # Example
/// ```
/// use ic_kit::ic;
///
/// #[derive(Default)]
/// struct S1 {
///     a: u64,
/// }
///
/// #[derive(Default)]
/// struct S2 {
///     a: u64,
/// }
///
///  ic::with_many(|(a, b): (&S1, &S2)| {
///     // Now we have access to both S1 and S2.
///     println!("S1: {}, S2: {}", a.a, b.a);
///  });
/// ```
pub fn with_many<A: BorrowMany, U, F: FnOnce(A) -> U>(callback: F) -> U {
    STORAGE.with(|storage| storage.with_many(callback))
}

/// Like [`crate::ic::with_mut`] but passes the mutable reference of multiple variables to the
/// closure as a tuple.
///
/// # Example
/// ```
/// use ic_kit::ic;
///
/// #[derive(Default)]
/// struct S1 {
///     a: u64,
/// }
///
/// #[derive(Default)]
/// struct S2 {
///     a: u64,
/// }
///
///  ic::with_many_mut(|(a, b): (&mut S1, &mut S2)| {
///     // Now we have access to both S1 and S2 and can mutate them.
///     a.a += 1;
///     b.a += 1;
///  });
/// ```
pub fn with_many_mut<A: BorrowMutMany, U, F: FnOnce(A) -> U>(callback: F) -> U {
    STORAGE.with(|storage| storage.with_many_mut(callback))
}
