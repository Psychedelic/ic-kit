use ic_types::Principal;
use std::panic::RefUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};

const REQUEST_ID: AtomicU64 = AtomicU64::new(0);

///  A request ID for a request that is coming to this canister from the outside.
pub type IncomingRequestId = RequestId;
/// A request ID for a request that this canister has submitted.
pub type OutgoingRequestId = RequestId;

/// An opaque request id.
#[derive(Hash, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct RequestId(u64);

impl RequestId {
    /// Create a new request id and return it.
    pub fn new() -> Self {
        Self(REQUEST_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// The entry method for a request.
pub enum EntryMode {
    Init,
    PreUpgrade,
    PostUpgrade,
    Heartbeat,
    InspectMessage,
    Update,
    Query,
    ReplyCallback,
    RejectCallback,
    CleanupCallback,
    CustomTask,
}

/// The canister's environment that should be used during a message.
pub struct Env {
    /// The type of the entry point that should be simulated, this enables trapping when a the
    /// method is calling a system api call that it should not be able to call during the
    /// execution of that entry point.
    pub entry_mode: EntryMode,
    /// The principal id of the sender.
    pub sender: Principal,
    /// The method to call. Only applies to update/query calls.
    pub method_name: Option<String>,
    /// The cycles provided to the canister during this call. In a reply or reject callback mode
    /// this is the amount of refunded cycles.
    pub cycles_available: u128,
    /// The arguments provided to the canister during this call.
    pub args: Vec<u8>,
    /// The reply rejection code. Default to `0`
    pub rejection_code: RejectionCode,
    /// The rejection message. Only applicable when `rejection_code != 0`
    pub rejection_message: String,
}

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

/// A message sent to a canister that trigger execution of a task on the canister's execution thread
/// based on the type of the message.
pub enum Message {
    /// A custom function that you want to be executed in the canister's execution thread.
    CustomTask {
        /// The request id of the incoming message.
        request_id: Option<IncomingRequestId>,
        /// the task handler that should be executed in the canister's execution thread.
        task: Box<dyn FnOnce() + Send + RefUnwindSafe>,
        /// The env to use for this custom execution.
        env: Env,
    },
    /// A normal IC request to the canister.
    Request {
        /// Only applicable if env.entry_mode is a reply/reject callback.
        reply_to: Option<OutgoingRequestId>,
        /// The env to use during the execution of this task.
        env: Env,
    },
}

/// A reply by the canister.
#[derive(Debug)]
pub enum CanisterReply {
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

impl Default for Env {
    fn default() -> Self {
        Env {
            entry_mode: EntryMode::CustomTask,
            sender: Principal::anonymous(),
            method_name: None,
            cycles_available: 0,
            args: vec![],
            rejection_code: RejectionCode::NoError,
            rejection_message: String::new(),
        }
    }
}

impl Env {
    /// Use the given entry mode in this env.
    pub fn with_entry_mode(mut self, mode: EntryMode) -> Self {
        self.entry_mode = mode;
        self
    }

    /// Provide this environment with the given principal id as the caller.
    pub fn with_sender(mut self, sender: Principal) -> Self {
        self.sender = sender;
        self
    }

    /// Provide the given env with the given method name to execute.
    pub fn with_method_name<S: Into<String>>(mut self, method_name: S) -> Self {
        self.method_name = Some(method_name.into());
        self
    }

    /// Provide the current env with the given amount of cycles to execute, in a reply or reject
    /// callback modes, this should be the amount that is refunded to the canister.
    pub fn with_cycles(mut self, cycles: u128) -> Self {
        self.cycles_available = cycles;
        self
    }

    /// The arguments in this environment, in a reply mode this is the data returned to the
    /// canister.
    pub fn with_args<A: Into<Vec<u8>>>(mut self, argument: A) -> Self {
        self.args = argument.into();
        self
    }

    /// Set this environment's rejection code the provided value, you must also set a rejection
    /// message if this is not equal to NoError.
    pub fn with_rejection_code(mut self, rejection_code: RejectionCode) -> Self {
        self.rejection_code = rejection_code;
        self
    }

    /// Set the rejection message on this env, only applicable if rejection_code is not zero.
    pub fn with_rejection_message<S: Into<String>>(mut self, rejection_message: S) -> Self {
        self.rejection_message = rejection_message.into();
        self
    }
}

impl Env {
    /// Return a name we can use to get the method from the symbol table.
    pub fn get_entry_point_name(&self) -> String {
        match &self.entry_mode {
            EntryMode::Init => "canister_init".to_string(),
            EntryMode::PreUpgrade => "canister_pre_upgrade".to_string(),
            EntryMode::PostUpgrade => "canister_post_upgrade".to_string(),
            EntryMode::Heartbeat => "canister_heartbeat".to_string(),
            EntryMode::InspectMessage => "canister_inspect_message".to_string(),
            EntryMode::Update => {
                format!(
                    "canister_update {}",
                    self.method_name.as_ref().unwrap_or(&String::new())
                )
            }
            EntryMode::Query => format!(
                "canister_query {}",
                self.method_name.as_ref().unwrap_or(&String::new())
            ),
            EntryMode::ReplyCallback => "reply callback".to_string(),
            EntryMode::RejectCallback => "reject callback".to_string(),
            EntryMode::CleanupCallback => "cleanup callback".to_string(),
            EntryMode::CustomTask => "ic-kit: custom".to_string(),
        }
    }
}
