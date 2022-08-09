use crate::futures;
use crate::futures::CallFuture;
use crate::ic::Cycles;
use crate::utils::arg_data_raw;
use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use candid::{decode_args, decode_one, encode_args, encode_one, CandidType, Principal};
use ic_kit_sys::ic0;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::error;
use std::fmt;

/// The result of `candid::encode_args(())` which is used as the default argument.
pub const CANDID_EMPTY_ARG: &[u8] = &[68, 73, 68, 76, 0, 0];

/// A call builder that let's you create an inter-canister call which can be then sent to the
/// destination.
pub struct CallBuilder {
    canister_id: Principal,
    method_name: String,
    payment: Cycles,
    arg: Option<Vec<u8>>,
}

/// Rejection code from calling another canister.
#[allow(missing_docs)]
#[repr(i32)]
#[derive(Debug, CandidType, Deserialize, Clone, Copy)]
pub enum RejectionCode {
    NoError = 0,
    SysFatal = 1,
    SysTransient = 2,
    DestinationInvalid = 3,
    CanisterReject = 4,
    CanisterError = 5,
    Unknown,
}

impl From<i32> for RejectionCode {
    fn from(code: i32) -> Self {
        match code {
            0 => RejectionCode::NoError,
            1 => RejectionCode::SysFatal,
            2 => RejectionCode::SysTransient,
            3 => RejectionCode::DestinationInvalid,
            4 => RejectionCode::CanisterReject,
            5 => RejectionCode::CanisterError,
            _ => RejectionCode::Unknown,
        }
    }
}

impl From<u32> for RejectionCode {
    fn from(code: u32) -> Self {
        RejectionCode::from(code as i32)
    }
}

#[derive(Debug)]
pub enum CallError {
    /// Indicates that the `ic0::call_perform` failed and the call is not queued.
    CouldNotSend,
    /// The rejection callback wsa called from the IC, the call failed with the given rejection
    /// code and message.
    Rejected(RejectionCode, String),
    /// The call happened successfully, but there was an error during deserialization of the
    /// response.
    /// The raw response is captured here.
    ResponseDeserializationError(Vec<u8>),
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CallError::CouldNotSend => f.write_str("Could not send message"),
            CallError::Rejected(c, m) => write!(f, "Call rejected (code={:?}): '{}'", c, m),
            CallError::ResponseDeserializationError(..) => {
                f.write_str("Could not deserialize the response.")
            }
        }
    }
}

impl error::Error for CallError {}

impl CallBuilder {
    /// Create a new call constructor, calling this method does nothing unless one of the perform
    /// methods are called.
    pub fn new<S: Into<String>>(canister_id: Principal, method_name: S) -> Self {
        Self {
            canister_id,
            method_name: method_name.into(),
            payment: 0,
            arg: None,
        }
    }

    /// Use the given candid tuple value as the argument.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    ///
    /// Use `clear_args` if you want to reset the arguments.
    pub fn with_args<T: ArgumentEncoder>(mut self, arguments: T) -> Self {
        assert!(self.arg.is_none(), "Call arguments can only be set once.");
        self.arg = Some(encode_args(arguments).unwrap());
        self
    }

    /// Shorthand for `with_args((argument, ))`.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    ///
    /// Use `clear_args` if you want to reset the arguments.
    pub fn with_arg<T: CandidType>(mut self, argument: T) -> Self {
        assert!(self.arg.is_none(), "Call arguments can only be set once.");
        self.arg = Some(encode_one(argument).unwrap());
        self
    }

    /// Set the raw argument that can be used for this call, this does not use candid to serialize
    /// the call argument and uses the provided raw buffer as the argument.
    ///
    /// Be sure that you know what you're doing when using this method.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    ///
    /// Use `clear_args` if you want to reset the arguments.
    pub fn with_arg_raw<A: Into<Vec<u8>>>(mut self, argument: A) -> Self {
        assert!(self.arg.is_none(), "Call arguments can only be set once.");
        self.arg = Some(argument.into());
        self
    }

    /// Clear any arguments set for this call. After calling this method you can call with_arg*
    /// methods again without the panic.
    pub fn clear_args(&mut self) {
        self.arg = None;
    }

    /// Set the payment amount for the canister. This will overwrite any previously added cycles
    /// to this call, use `add_payment` if you want to increment the amount of used cycles in
    /// this call.
    ///
    /// # Safety
    ///
    /// Be sure that your canister has the provided amount of cycles upon performing the call,
    /// since any of the perform methods will just trap the canister if the provided payment
    /// amount is larger than the amount of canister's balance.
    pub fn with_payment(mut self, payment: Cycles) -> Self {
        self.payment = payment;
        self
    }

    /// Add the given provided amount of cycles to the cycles already provided to this call.
    pub fn add_payment(mut self, payment: Cycles) -> Self {
        self.payment += payment;
        self
    }

    /// Should be called after the `ic0::call_new` to set the call arguments.
    #[inline(always)]
    unsafe fn ic0_internal_call_perform(&self) -> i32 {
        #[cfg(not(feature = "experimental-cycles128"))]
        ic0::call_cycles_add(self.payment as i64);

        #[cfg(feature = "experimental-cycles128")]
        if self.payment > 0 && self.payment < (u64::MAX as u128) {
            ic0::call_cycles_add(self.payment as i64);
        } else if self.payment > 0 {
            let high = (self.payment >> 64) as u64 as i64;
            let low = (self.payment & (1 << 64)) as u64 as i64;
            ic0::call_cycles_add128(high, low);
        }

        let args_raw = self.arg.as_deref().unwrap_or(CANDID_EMPTY_ARG);

        if !args_raw.is_empty() {
            ic0::call_data_append(args_raw.as_ptr() as isize, args_raw.len() as isize);
        }

        ic0::call_perform()
    }

    /// Perform a call when you do not care about the response in anyway. We advise you to use this
    /// method when you can since it is probably cheaper.
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    pub fn perform_one_way(self) {
        let callee = self.canister_id.as_slice();
        let method = self.method_name.as_str();

        unsafe {
            ic0::call_new(
                callee.as_ptr() as isize,
                callee.len() as isize,
                method.as_ptr() as isize,
                method.len() as isize,
                -1,
                -1,
                -1,
                -1,
            );

            self.ic0_internal_call_perform();
        }
    }

    /// Perform the call and return a future that can will be resolved in any of the callbacks.
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    #[must_use]
    fn perform_internal(&self) -> CallFuture {
        let future = unsafe {
            let future = futures::call_new(self.canister_id, self.method_name.as_str());
            let e_code = self.ic0_internal_call_perform();

            if e_code != 0 {
                future.mark_ready()
            } else {
                future
            }
        };

        future
    }

    /// Use this method when you want to perform a call and only care about the delivery status
    /// of the call and don't need the returned buffer in anyway.
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    pub async fn perform_rejection(&self) -> Result<(), CallError> {
        let future = self.perform_internal();

        // if the future is already ready, it indicates a `ic0::call_perform` non-zero response.
        if future.is_ready() {
            return Err(CallError::CouldNotSend);
        }

        // await for the call to comeback.
        future.await;

        let rejection_code = unsafe { ic0::msg_reject_code() };
        if rejection_code == 0 {
            return Ok(());
        }

        let rejection_message_size = unsafe { ic0::msg_reject_msg_size() } as usize;
        let mut bytes = vec![0u8; rejection_message_size];
        unsafe {
            ic0::msg_reject_msg_copy(
                bytes.as_mut_ptr() as isize,
                0,
                rejection_message_size as isize,
            );
        }

        Err(CallError::Rejected(
            rejection_code.into(),
            String::from_utf8_lossy(&bytes).to_string(),
        ))
    }

    /// Perform the call and return the raw response buffer without decoding it.
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    pub async fn perform_raw(&self) -> Result<Vec<u8>, CallError> {
        self.perform_rejection().await?;
        Ok(arg_data_raw())
    }

    /// Perform the call and return a future which will resolve to the candid decoded response. Or
    /// any of the errors that might happen, consider looking at other alternatives of this method
    /// as well if you don't care about the response or want the raw/non-decoded response.
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    pub async fn perform<R: for<'a> ArgumentDecoder<'a>>(&self) -> Result<R, CallError> {
        let bytes = self.perform_raw().await?;

        match decode_args(&bytes) {
            Err(_) => Err(CallError::ResponseDeserializationError(bytes)),
            Ok(r) => Ok(r),
        }
    }

    /// Perform the call and return a future which will resolve to the candid decoded response.
    /// Unlink perform, this method only expects a result with one argument from the canister,
    /// and decodes the arguments using the candid's decode_one.
    ///
    ///
    /// # Traps
    ///
    /// This method traps if the amount determined in the `payment` is larger than the canister's
    /// balance at the time of invocation.
    pub async fn perform_one<T>(&self) -> Result<T, CallError>
    where
        T: DeserializeOwned + CandidType,
    {
        let bytes = self.perform_raw().await?;

        match decode_one(&bytes) {
            Err(_) => Err(CallError::ResponseDeserializationError(bytes)),
            Ok(r) => Ok(r),
        }
    }
}
