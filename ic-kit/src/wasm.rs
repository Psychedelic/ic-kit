use ic_cdk;
use ic_cdk::export::candid::utils::{ArgumentDecoder, ArgumentEncoder};
use ic_cdk::export::{candid, Principal};

use crate::storage::Storage;
use crate::{CallResponse, Context, StableMemoryError};

static mut CONTEXT: Option<IcContext> = None;

/// A singleton context that is used in the actual IC environment.
pub struct IcContext {
    /// The storage for this context.
    storage: Storage,
}

impl IcContext {
    /// Return a mutable reference to the context.
    #[inline(always)]
    pub fn context() -> &'static mut IcContext {
        unsafe {
            if let Some(ctx) = &mut CONTEXT {
                ctx
            } else {
                CONTEXT = Some(IcContext {
                    storage: Storage::default(),
                });
                IcContext::context()
            }
        }
    }

    #[inline(always)]
    fn as_mut(&self) -> &mut Self {
        unsafe {
            let const_ptr = self as *const Self;
            let mut_ptr = const_ptr as *mut Self;
            &mut *mut_ptr
        }
    }
}

impl Context for IcContext {
    #[inline(always)]
    fn trap(&self, message: &str) -> ! {
        ic_cdk::api::trap(message);
    }

    #[inline(always)]
    fn print<S: AsRef<str>>(&self, s: S) {
        ic_cdk::api::print(s)
    }

    #[inline(always)]
    fn id(&self) -> Principal {
        ic_cdk::id()
    }

    #[inline(always)]
    fn time(&self) -> u64 {
        ic_cdk::api::time()
    }

    #[inline(always)]
    fn balance(&self) -> u64 {
        ic_cdk::api::canister_balance()
    }

    #[inline(always)]
    fn caller(&self) -> Principal {
        ic_cdk::api::caller()
    }

    #[inline(always)]
    fn msg_cycles_available(&self) -> u64 {
        ic_cdk::api::call::msg_cycles_available()
    }

    #[inline(always)]
    fn msg_cycles_accept(&self, amount: u64) -> u64 {
        ic_cdk::api::call::msg_cycles_accept(amount)
    }

    #[inline(always)]
    fn msg_cycles_refunded(&self) -> u64 {
        ic_cdk::api::call::msg_cycles_refunded()
    }

    #[inline(always)]
    fn stable_store<T>(&self, data: T) -> Result<(), candid::Error>
    where
        T: ArgumentEncoder,
    {
        ic_cdk::storage::stable_save(data)
    }

    #[inline(always)]
    fn stable_restore<T>(&self) -> Result<T, String>
    where
        T: for<'de> ArgumentDecoder<'de>,
    {
        ic_cdk::storage::stable_restore()
    }

    #[inline(always)]
    fn call_raw<S: Into<String>>(
        &'static self,
        id: Principal,
        method: S,
        args_raw: Vec<u8>,
        cycles: u64,
    ) -> CallResponse<Vec<u8>> {
        let method = method.into();
        Box::pin(async move {
            ic_cdk::api::call::call_raw(id, &method, args_raw.as_slice(), cycles).await
        })
    }

    #[inline(always)]
    fn set_certified_data(&self, data: &[u8]) {
        ic_cdk::api::set_certified_data(data);
    }

    #[inline(always)]
    fn data_certificate(&self) -> Option<Vec<u8>> {
        ic_cdk::api::data_certificate()
    }

    #[inline(always)]
    fn spawn<F: 'static + std::future::Future<Output = ()>>(&mut self, future: F) {
        ic_cdk::spawn(future)
    }

    #[inline(always)]
    fn stable_size(&self) -> u32 {
        ic_cdk::api::stable::stable_size()
    }

    #[inline(always)]
    fn stable_grow(&self, new_pages: u32) -> Result<u32, StableMemoryError> {
        ic_cdk::api::stable::stable_grow(new_pages).map_err(|_| StableMemoryError())
    }

    #[inline(always)]
    fn stable_write(&self, offset: u32, buf: &[u8]) {
        ic_cdk::api::stable::stable_write(offset, buf)
    }

    #[inline(always)]
    fn stable_read(&self, offset: u32, buf: &mut [u8]) {
        ic_cdk::api::stable::stable_read(offset, buf)
    }

    #[inline(always)]
    fn with<T: 'static + Default, U, F: FnOnce(&T) -> U>(&self, callback: F) -> U {
        self.as_mut().storage.with(callback)
    }

    #[inline(always)]
    fn maybe_with<T: 'static, U, F: FnOnce(&T) -> U>(&self, callback: F) -> Option<U> {
        self.as_mut().storage.maybe_with(callback)
    }

    #[inline(always)]
    fn with_mut<T: 'static + Default, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> U {
        self.as_mut().storage.with_mut(callback)
    }

    #[inline(always)]
    fn maybe_with_mut<T: 'static, U, F: FnOnce(&mut T) -> U>(&self, callback: F) -> Option<U> {
        self.as_mut().storage.maybe_with_mut(callback)
    }

    #[inline(always)]
    fn take<T: 'static>(&self) -> Option<T> {
        self.as_mut().storage.take()
    }

    #[inline(always)]
    fn swap<T: 'static>(&self, value: T) -> Option<T> {
        self.as_mut().storage.swap(value)
    }
}
