use candid::utils::{ArgumentDecoder, ArgumentEncoder};
use candid::{encode_args, encode_one, CandidType, Principal};

pub struct Call {
    canister_id: Principal,
    method_name: String,
    payment: u128,
    // default = vec![68, 73, 68, 76, 0, 0],
    arg: Option<Vec<u8>>,
}

struct CallResponse {
    rejection_code: u32,
}

impl Call {
    /// Create a new call constructor, calling this method does nothing unless one of the perform
    /// methods are called.
    pub fn new(canister_id: Principal, method_name: String) -> Self {
        Self {
            canister_id,
            method_name,
            payment: 0,
            arg: None,
        }
    }

    /// Set the arguments for this call.
    pub fn with_args<T: ArgumentEncoder>(mut self, arguments: T) -> Self {
        assert!(self.arg.is_none(), "Call arguments can only be set once.");
        self.arg = Some(encode_args(arguments).unwrap());
        self
    }

    /// Set one argument for this call.
    pub fn with_arg<T: CandidType>(mut self, argument: T) -> Self {
        assert!(self.arg.is_none(), "Call arguments can only be set once.");
        self.arg = Some(encode_one(argument).unwrap());
        self
    }

    /// Set the payment amount for the canister.
    pub fn with_payment(mut self, payment: u128) -> Self {
        self.payment += payment;
        self
    }

    /// Perform the call and return a future for the response.
    #[must_use]
    pub fn perform(self) {}

    /// Perform a call that we do not care about its response.
    pub fn perform_one_way(self) {}
}

#[test]
fn x() {
    println!("{:?}", encode_args(()));
}
