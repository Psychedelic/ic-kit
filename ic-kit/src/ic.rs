use crate::candid::utils::{ArgumentDecoder, ArgumentEncoder};
pub use crate::stable::*;
use crate::{candid, CallResponse, Context, Principal, StableMemoryError};

#[inline(always)]
fn get_context() -> &'static mut impl Context {
    #[cfg(not(target_family = "wasm"))]
    return crate::inject::get_context();
    #[cfg(target_family = "wasm")]
    return crate::wasm::IcContext::context();
}

/// Trap the code.
#[inline(always)]
pub fn trap(message: &str) -> ! {
    get_context().trap(message)
}

/// Print a message.
#[inline(always)]
pub fn print<S: AsRef<str>>(s: S) {
    get_context().print(s)
}

/// ID of the current canister.
#[inline(always)]
pub fn id() -> Principal {
    get_context().id()
}

/// The time in nanoseconds.
#[inline(always)]
pub fn time() -> u64 {
    get_context().time()
}

/// The balance of the canister.
#[inline(always)]
pub fn balance() -> u64 {
    get_context().balance()
}

/// The caller who has invoked this method on the canister.
#[inline(always)]
pub fn caller() -> Principal {
    get_context().caller()
}

/// Return the number of available cycles that is sent by the caller.
#[inline(always)]
pub fn msg_cycles_available() -> u64 {
    get_context().msg_cycles_available()
}

/// Accept the given amount of cycles, returns the actual amount of accepted cycles.
#[inline(always)]
pub fn msg_cycles_accept(amount: u64) -> u64 {
    get_context().msg_cycles_accept(amount)
}

/// Return the cycles that were sent back by the canister that was just called.
/// This method should only be called right after an inter-canister call.
#[inline(always)]
pub fn msg_cycles_refunded() -> u64 {
    get_context().msg_cycles_refunded()
}

/// Store the given data to the stable storage.
#[inline(always)]
pub fn stable_store<T>(data: T) -> Result<(), candid::Error>
where
    T: ArgumentEncoder,
{
    get_context().stable_store(data)
}

/// Restore the data from the stable storage. If the data is not already stored the None value
/// is returned.
#[inline(always)]
pub fn stable_restore<T>() -> Result<T, String>
where
    T: for<'de> ArgumentDecoder<'de>,
{
    get_context().stable_restore()
}

/// Perform a call.
#[inline(always)]
pub fn call_raw<S: Into<String>>(
    id: Principal,
    method: S,
    args_raw: Vec<u8>,
    cycles: u64,
) -> CallResponse<Vec<u8>> {
    get_context().call_raw(id, method, args_raw, cycles)
}

/// Perform the call and return the response.
#[inline(always)]
pub fn call<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
    id: Principal,
    method: S,
    args: T,
) -> CallResponse<R> {
    get_context().call_with_payment(id, method, args, 0)
}

#[inline(always)]
pub fn call_with_payment<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
    id: Principal,
    method: S,
    args: T,
    cycles: u64,
) -> CallResponse<R> {
    get_context().call_with_payment(id, method, args, cycles)
}

/// Set the certified data of the canister, this method traps if data.len > 32.
#[inline(always)]
pub fn set_certified_data(data: &[u8]) {
    get_context().set_certified_data(data)
}

/// Returns the data certificate authenticating certified_data set by this canister.
#[inline(always)]
pub fn data_certificate() -> Option<Vec<u8>> {
    get_context().data_certificate()
}

/// Execute a future without blocking the current call.
#[inline(always)]
pub fn spawn<F: 'static + std::future::Future<Output = ()>>(future: F) {
    get_context().spawn(future)
}

/// Returns the current size of the stable memory in WebAssembly pages.
/// (One WebAssembly page is 64KiB)
#[inline(always)]
pub fn stable_size() -> u32 {
    get_context().stable_size()
}

/// Tries to grow the memory by new_pages many pages containing zeroes.
/// This system call traps if the previous size of the memory exceeds 2^32 bytes.
/// Errors if the new size of the memory exceeds 2^32 bytes or growing is unsuccessful.
/// Otherwise, it grows the memory and returns the previous size of the memory in pages.
#[inline(always)]
pub fn stable_grow(new_pages: u32) -> Result<u32, StableMemoryError> {
    get_context().stable_grow(new_pages)
}

/// Writes data to the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_write(offset: u32, buf: &[u8]) {
    get_context().stable_write(offset, buf)
}

/// Reads data from the stable memory location specified by an offset.
#[inline(always)]
pub fn stable_read(offset: u32, buf: &mut [u8]) {
    get_context().stable_read(offset, buf)
}

/// Returns a copy of the stable memory.
///
/// This will map the whole memory (even if not all of it has been written to), we don't recommend
/// using such a method as this is an expensive read of the entire stable storage to the heap.
///
/// Only use it with caution.
pub fn stable_bytes() -> Vec<u8> {
    let size = (stable_size() as usize) << 16;
    let mut vec = Vec::with_capacity(size);
    unsafe {
        vec.set_len(size);
    }

    stable_read(0, vec.as_mut_slice());

    vec
}

/// Pass an immutable reference to the value associated with the given type to the closure.
///
/// If no value is currently associated to the type `T`, this method will insert the default
/// value in its place before invoking the callback. Use `maybe_with` if you don't want the
/// default value to be inserted or if your type does not implement the [`Default`] trait.
///
/// This is a safe replacement for the previously known `ic_kit::ic::get` API, and you can use it
/// instead of `lazy_static` or `local_thread`.
///
/// # Example
///
/// ```
/// use ic_kit::*;
///
/// #[derive(Default)]
/// struct Counter {
///     count: u64
/// }
///
/// impl Counter {
///     fn get(&self) -> u64 {
///         self.count
///     }
/// }
///
/// MockContext::new()
///     .with_data(Counter { count: 17 })
///     .inject();
///
/// assert_eq!(ic::with(Counter::get), 17);
/// ```
pub fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(callback: F) -> U {
    get_context().with(callback)
}

/// Like [`with`], but does not initialize the data with the default value and simply returns None,
/// if there is no value associated with the type.
pub fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(callback: F) -> Option<U> {
    get_context().maybe_with(callback)
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
/// ```
/// use ic_kit::*;
///
/// #[derive(Default)]
/// struct Counter {
///     count: u64
/// }
///
/// impl Counter {
///     fn increment(&mut self) -> u64 {
///         self.count += 1;
///         self.count
///     }
/// }
///
/// MockContext::new()
///     .with_data(Counter { count: 17 })
///     .inject();
///
/// assert_eq!(ic::with_mut(Counter::increment), 18);
/// ```
pub fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(callback: F) -> U {
    get_context().with_mut(callback)
}

/// Like [`with_mut`], but does not initialize the data with the default value and simply returns
/// None, if there is no value associated with the type.
pub fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(callback: F) -> Option<U> {
    get_context().maybe_with_mut(callback)
}

/// Remove the current value associated with the type and return it.
pub fn remove<T: 'static>() -> Option<T> {
    get_context().remove::<T>()
}

/// Swaps the value associated with type `T` with the given value, returns the old one.
pub fn swap<T: 'static>(value: T) -> Option<T> {
    get_context().swap(value)
}

/// Store the given data to the storage.
#[inline(always)]
#[deprecated(
    since = "0.4.8",
    note = "Unsafe memory methods are deprecated, use the safer ic_kit::ic::swap method."
)]
pub fn store<T: 'static>(data: T) {
    get_context().store(data)
}

/// Return the data that does not implement [`Default`].
#[inline(always)]
#[deprecated(
    since = "0.4.8",
    note = "Unsafe memory methods are deprecated, use the safer ic_kit::ic::maybe_with method."
)]
pub fn get_maybe<T: 'static>() -> Option<&'static T> {
    get_context().get_maybe()
}

/// Return the data associated with the given type. If the data is not present the default
/// value of the type is returned.
#[inline(always)]
#[deprecated(
    since = "0.4.8",
    note = "Unsafe memory methods are deprecated, use the safer ic_kit::ic::with method."
)]
pub fn get<T: 'static + Default>() -> &'static T {
    get_context().get_mut()
}

/// Return a mutable reference to the given data type, if the data is not present the default
/// value of the type is constructed and stored. The changes made to the data during updates
/// is preserved.
#[inline(always)]
#[deprecated(
    since = "0.4.8",
    note = "Unsafe memory methods are deprecated, use the safer ic_kit::ic::with method."
)]
pub fn get_mut<T: 'static + Default>() -> &'static mut T {
    get_context().get_mut()
}

/// Remove the data associated with the given data type.
#[inline(always)]
#[deprecated(
    since = "0.4.8",
    note = "Unsafe memory methods are deprecated, use the safer ic_kit::ic::remove method."
)]
pub fn delete<T: 'static + Default>() -> bool {
    get_context().delete::<T>()
}
