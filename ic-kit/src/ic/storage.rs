use crate::storage::Storage;

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
///
/// # Example
///
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
