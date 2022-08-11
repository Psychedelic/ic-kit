use std::error;
use std::fmt;

/// The result of `candid::encode_args(())` which is used as the default argument.
pub const CANDID_EMPTY_ARG: &[u8] = &[68, 73, 68, 76, 0, 0];

/// Rejection code from calling another canister.
#[allow(missing_docs)]
#[repr(i32)]
#[derive(Debug, Clone, Copy)]
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

/// A possible error value when dealing with stable memory.
#[derive(Debug)]
pub enum StableMemoryError {
    /// No more stable memory could be allocated.
    OutOfMemory,
    /// Attempted to read more stable memory than had been allocated.
    OutOfBounds,
}

impl fmt::Display for StableMemoryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::OutOfMemory => f.write_str("Out of memory"),
            Self::OutOfBounds => f.write_str("Read exceeds allocated memory"),
        }
    }
}

impl error::Error for StableMemoryError {}
