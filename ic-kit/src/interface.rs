use std::future::Future;
use std::pin::Pin;

use ic_cdk::api::call::CallResult;
use ic_cdk::export::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use ic_cdk::export::candid::{decode_args, encode_args};
use ic_cdk::export::{candid, Principal};

pub type CallResponse<T> = Pin<Box<dyn Future<Output = CallResult<T>>>>;

/// A possible error value when dealing with stable memory.
#[derive(Debug)]
pub struct StableMemoryError();

pub trait Context {
    /// Trap the code.
    fn trap(&self, message: &str) -> !;

    /// Print a message.
    fn print<S: AsRef<str>>(&self, s: S);

    /// ID of the current canister.
    fn id(&self) -> Principal;

    /// The time in nanoseconds.
    fn time(&self) -> u64;

    /// The balance of the canister.
    fn balance(&self) -> u64;

    /// The caller who has invoked this method on the canister.
    fn caller(&self) -> Principal;

    /// Return the number of available cycles that is sent by the caller.
    fn msg_cycles_available(&self) -> u64;

    /// Accept the given amount of cycles, returns the actual amount of accepted cycles.
    fn msg_cycles_accept(&self, amount: u64) -> u64;

    /// Return the cycles that were sent back by the canister that was just called.
    /// This method should only be called right after an inter-canister call.
    fn msg_cycles_refunded(&self) -> u64;

    /// Store the given data to the stable storage.
    fn stable_store<T>(&self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder;

    /// Restore the data from the stable storage. If the data is not already stored the None value
    /// is returned.
    fn stable_restore<T>(&self) -> Result<T, String>
    where
        T: for<'de> ArgumentDecoder<'de>;

    /// Perform a call.
    fn call_raw<S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args_raw: Vec<u8>,
        cycles: u64,
    ) -> CallResponse<Vec<u8>>;

    /// Perform the call and return the response.
    #[inline(always)]
    fn call<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args: T,
    ) -> CallResponse<R> {
        self.call_with_payment(id, method, args, 0)
    }

    #[inline(always)]
    fn call_with_payment<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>, S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args: T,
        cycles: u64,
    ) -> CallResponse<R> {
        let args_raw = encode_args(args).expect("Failed to encode arguments.");
        let method = method.into();
        Box::pin(async move {
            let bytes = self.call_raw(id, method, args_raw, cycles).await?;
            decode_args(&bytes).map_err(|err| panic!("{:?}", err))
        })
    }

    /// Set the certified data of the canister, this method traps if data.len > 32.
    fn set_certified_data(&self, data: &[u8]);

    /// Returns the data certificate authenticating certified_data set by this canister.
    fn data_certificate(&self) -> Option<Vec<u8>>;

    /// Execute a future without blocking the current call.
    fn spawn<F: 'static + std::future::Future<Output = ()>>(&mut self, future: F);

    /// Returns the current size of the stable memory in WebAssembly pages.
    /// (One WebAssembly page is 64KiB)
    fn stable_size(&self) -> u32;

    /// Tries to grow the memory by new_pages many pages containing zeroes.
    /// This system call traps if the previous size of the memory exceeds 2^32 bytes.
    /// Errors if the new size of the memory exceeds 2^32 bytes or growing is unsuccessful.
    /// Otherwise, it grows the memory and returns the previous size of the memory in pages.
    fn stable_grow(&self, new_pages: u32) -> Result<u32, StableMemoryError>;

    /// Writes data to the stable memory location specified by an offset.
    fn stable_write(&self, offset: u32, buf: &[u8]);

    /// Reads data from the stable memory location specified by an offset.
    fn stable_read(&self, offset: u32, buf: &mut [u8]);

    /// Pass an immutable reference of data with type `T` to the callback, stores the default value
    /// if not present, and return the transformation.
    fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(&self, callback: F) -> U;

    /// Pass an immutable reference of data with type `T` to the callback, and return the mapped
    /// value.
    fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(&self, callback: F) -> Option<U>;

    /// Pass the mutable reference of data with type `T` to the callback, stores the default value
    /// if not present, and return the transformation.
    fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> U;

    /// Pass the mutable reference of data with type `T` to the callback, and return the callback's
    /// result.
    fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> Option<U>;

    /// Remove the data associated with the given type, and return it.
    fn remove<T: 'static>(&self) -> Option<T>;

    /// Replaced the stored value of type `T` with the new provided one and return the old one if
    /// any.
    fn swap<T: 'static>(&self, value: T) -> Option<T>;

    /// See [ic::store](crate::ic::store)
    #[deprecated]
    fn store<T: 'static>(&self, data: T);

    /// See [ic::store](crate::ic::get_maybe)
    #[deprecated]
    fn get_maybe<T: 'static>(&self) -> Option<&T>;

    /// See [ic::store](crate::ic::get)
    #[deprecated]
    fn get<T: 'static + Default>(&self) -> &T;

    /// See [ic::store](crate::ic::get_mut)
    #[deprecated]
    fn get_mut<T: 'static + Default>(&self) -> &mut T;

    /// See [ic::store](crate::ic::delete)
    #[deprecated]
    fn delete<T: 'static + Default>(&self) -> bool;
}
