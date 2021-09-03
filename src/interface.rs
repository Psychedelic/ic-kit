use ic_cdk::api::call::{call_raw, CallResult};
use ic_cdk::export::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use ic_cdk::export::candid::{decode_args, encode_args, CandidType};
use ic_cdk::export::Principal;
use std::future::Future;
use std::pin::Pin;

pub type CallResponse<T> = Pin<Box<dyn Future<Output = CallResult<T>>>>;

pub trait Context {
    /// ID of the current canister.
    fn id(&self) -> Principal;

    /// The caller who has invoked this method on the canister.
    fn caller(&self) -> Principal;

    /// Return the number of available cycles that is sent by the caller.
    fn cycles_available(&self) -> u64;

    /// Accept the given amount of cycles, returns the actual amount of accepted cycles.
    fn cycles_accept(&mut self, amount: u64) -> u64;

    /// Return the data associated with the given type. If the data is not present the default
    /// value of the type is returned.
    fn get<T: Default>(&self) -> &T;

    /// Return a mutable reference to the given data type, if the data is not present the default
    /// value of the type is constructed and stored. The changes made to the data during updates
    /// is preserved.
    fn get_mut<T: Default>(&mut self) -> &mut T;

    /// Store the given data to the stable storage.
    fn stable_store<T>(&mut self, data: T);

    /// Restore the data from the stable storage. If the data is not already stored the None value
    /// is returned.
    fn stable_restore<T>(&self) -> Option<T>;

    /// Perform a call
    fn call_raw(
        &self,
        id: Principal,
        method: &str,
        args_raw: Vec<u8>,
        cycles: u64,
    ) -> CallResponse<Vec<u8>>;

    /// Perform the call and return the response.
    #[inline]
    fn call<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>>(
        &self,
        id: Principal,
        method: &str,
        args: T,
    ) -> CallResponse<R> {
        self.call_with_payment(id, method, args, 0)
    }

    /// Perform the call and return the response.
    #[inline]
    fn call_with_payment<T: ArgumentEncoder, R: for<'a> ArgumentDecoder<'a>>(
        &self,
        id: Principal,
        method: &str,
        args: T,
        cycles: u64,
    ) -> CallResponse<R> {
        let args_raw = encode_args(args).expect("Failed to encode arguments.");
        Box::pin(async move {
            let bytes = self.call_raw(id, method, args_raw, cycles).await?;
            decode_args(&bytes).map_err(|err| panic!(&format!("{:?}", err)))
        })
    }

    /// Return the cycles that were sent back by the canister that was just called.
    /// This method should only be called right after an inter-canister call.
    fn cycles_refunded(&self) -> u64;
}
