use crate::types::*;
use crate::Replica;
use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use candid::{decode_args, decode_one, encode_args, encode_one, CandidType};
use ic_kit_sys::types::{CallError, RejectionCode, CANDID_EMPTY_ARG};
use ic_types::Principal;
use serde::de::DeserializeOwned;

/// A CallBuilder for a replica.
#[derive(Clone)]
pub struct CallBuilder<'a> {
    replica: &'a Replica,
    canister_id: Principal,
    method_name: String,
    sender: Principal,
    payment: u128,
    arg: Option<Vec<u8>>,
}

/// A reply by the canister.
#[derive(Debug)]
pub enum CallReply {
    Reply {
        data: Vec<u8>,
        cycles_refunded: u128,
    },
    Reject {
        rejection_code: RejectionCode,
        rejection_message: String,
        cycles_refunded: u128,
    },
}

impl<'a> CallBuilder<'a> {
    /// Create a new call builder for the given type.
    pub fn new(replica: &'a Replica, canister_id: Principal, method_name: String) -> Self {
        Self {
            replica,
            canister_id,
            sender: Principal::anonymous(),
            method_name,
            payment: 0,
            arg: None,
        }
    }

    /// Use the given candid tuple value as the argument for this mock call.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    pub fn with_args<T: ArgumentEncoder>(mut self, arguments: T) -> Self {
        assert!(self.arg.is_none(), "Arguments may only be set once.");
        self.arg = Some(encode_args(arguments).unwrap());
        self
    }

    /// Shorthand for `with_args((argument, ))` to pass tuples with only one element to the call.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    pub fn with_arg<T: CandidType>(mut self, argument: T) -> Self {
        assert!(self.arg.is_none(), "Arguments may only be set once.");
        self.arg = Some(encode_one(argument).unwrap());
        self
    }

    /// Pass the given raw buffer as the call argument, this does not perform any serialization on
    /// the data.
    ///
    /// # Panics
    ///
    /// This method panics if the argument for this call is already set via a prior
    /// call to any of the `with_args`, `with_arg` or `with_arg_raw`.
    pub fn with_arg_raw<A: Into<Vec<u8>>>(mut self, argument: A) -> Self {
        assert!(self.arg.is_none(), "Arguments may only be set once.");
        self.arg = Some(argument.into());
        self
    }

    /// Use the given amount of cycles for this mock call.
    pub fn with_payment(mut self, cycles: u128) -> Self {
        self.payment = cycles;
        self
    }

    /// Make the call from this sender.
    pub fn with_caller<I: Into<Principal>>(mut self, caller: I) -> Self {
        self.sender = caller.into();
        self
    }

    /// Perform the call and returns the reply from the canister.
    pub async fn perform(&self) -> CallReply {
        self.replica.perform_call(self.into()).await
    }
}

impl CallReply {
    /// Convert the reply to a message that can be delivered to a canister.
    pub(crate) fn to_message(self, reply_to: OutgoingRequestId) -> CanisterMessage {
        match self {
            CallReply::Reply {
                data,
                cycles_refunded,
            } => CanisterMessage::Reply {
                reply_to,
                env: Env::default()
                    .with_entry_mode(EntryMode::ReplyCallback)
                    .with_raw_args(data)
                    .with_cycles_refunded(cycles_refunded),
            },
            CallReply::Reject {
                rejection_code,
                rejection_message,
                cycles_refunded,
            } => CanisterMessage::Reply {
                reply_to,
                env: Env::default()
                    .with_entry_mode(EntryMode::RejectCallback)
                    .with_cycles_refunded(cycles_refunded)
                    .with_rejection_code(rejection_code)
                    .with_rejection_message(rejection_message),
            },
        }
    }

    /// Return the raw response bytes from this call.
    pub fn bytes(&self) -> Result<&[u8], CallError> {
        self.into()
    }

    /// Try to decode the response to the provided candid tuple.
    pub fn decode<T: for<'a> ArgumentDecoder<'a>>(&self) -> Result<T, CallError> {
        let bytes = self.bytes()?;
        match decode_args(bytes) {
            Err(_) => Err(CallError::ResponseDeserializationError(bytes.to_vec())),
            Ok(r) => Ok(r),
        }
    }

    /// Tries to decode a single argument.
    pub fn decode_one<T>(&self) -> Result<T, CallError>
    where
        T: DeserializeOwned + CandidType,
    {
        let bytes = self.bytes()?;
        match decode_one(bytes) {
            Err(_) => Err(CallError::ResponseDeserializationError(bytes.to_vec())),
            Ok(r) => Ok(r),
        }
    }

    /// Return the rejection code from this call, returns `RejectionCode::NoError` when the call
    /// succeed.
    pub fn rejection_code(&self) -> RejectionCode {
        match &self {
            CallReply::Reply { .. } => RejectionCode::NoError,
            CallReply::Reject { rejection_code, .. } => *rejection_code,
        }
    }

    /// Returns the possible rejection message.
    pub fn rejection_message(&self) -> Option<&str> {
        match &self {
            CallReply::Reply { .. } => None,
            CallReply::Reject {
                rejection_message, ..
            } => Some(rejection_message.as_str()),
        }
    }

    //// Returns the number of cycles refunded from this canister.
    pub fn cycles_refunded(&self) -> u128 {
        match &self {
            CallReply::Reply {
                cycles_refunded, ..
            } => *cycles_refunded,
            CallReply::Reject {
                cycles_refunded, ..
            } => *cycles_refunded,
        }
    }

    /// Returns true if the call was okay.
    pub fn is_ok(&self) -> bool {
        match &self {
            CallReply::Reply { .. } => true,
            CallReply::Reject { .. } => false,
        }
    }

    /// Returns true if the call was rejected.
    pub fn is_error(&self) -> bool {
        match &self {
            CallReply::Reply { .. } => false,
            CallReply::Reject { .. } => true,
        }
    }

    /// Assert the response is okay.
    pub fn assert_ok(&self) {
        assert!(self.is_ok(), "The call was rejected.");
    }

    /// Assert the response is a rejection.
    pub fn assert_error(&self) {
        assert!(self.is_error(), "Expected a rejection, but got a reply.");
    }
}

impl<'a> From<&'a CallReply> for Result<&'a [u8], CallError> {
    fn from(reply: &'a CallReply) -> Self {
        match reply {
            CallReply::Reply { data, .. } => Ok(data.as_slice()),
            CallReply::Reject {
                rejection_code,
                rejection_message,
                ..
            } => Err(CallError::Rejected(
                *rejection_code,
                rejection_message.clone(),
            )),
        }
    }
}

impl<'a> From<&'a CallBuilder<'a>> for CanisterCall {
    fn from(builder: &'a CallBuilder) -> Self {
        CanisterCall {
            sender: builder.sender,
            request_id: RequestId::new(),
            callee: builder.canister_id,
            method: builder.method_name.clone(),
            payment: builder.payment,
            arg: builder
                .arg
                .clone()
                .unwrap_or_else(|| CANDID_EMPTY_ARG.to_vec()),
        }
    }
}
