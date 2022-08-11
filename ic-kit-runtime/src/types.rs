use candid::utils::ArgumentEncoder;
use candid::{encode_args, encode_one, CandidType};
use ic_kit_sys::types::{RejectionCode, CANDID_EMPTY_ARG};
use ic_types::Principal;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static REQUEST_ID: AtomicU64 = AtomicU64::new(0);

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
        Self(REQUEST_ID.fetch_add(1, Ordering::SeqCst))
    }
}

/// The entry method for a request.
#[derive(Debug, PartialEq, Copy, Clone)]
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
    /// Determines the canister' balance.
    pub balance: u128,
    /// The type of the entry point that should be simulated, this enables trapping when a the
    /// method is calling a system api call that it should not be able to call during the
    /// execution of that entry point.
    pub entry_mode: EntryMode,
    /// The principal id of the sender.
    pub sender: Principal,
    /// The method to call. Only applies to update/query calls.
    pub method_name: Option<String>,
    /// The cycles provided to the canister during this call.
    pub cycles_available: u128,
    /// The amount of refunded cycles.
    pub cycles_refunded: u128,
    /// The arguments provided to the canister during this call.
    pub args: Vec<u8>,
    /// The reply rejection code. Default to `0`
    pub rejection_code: RejectionCode,
    /// The rejection message. Only applicable when `rejection_code != 0`
    pub rejection_message: String,
    /// The current time in nanoseconds.
    pub time: u64,
}

pub type TaskFn = Box<dyn FnOnce() + Send + RefUnwindSafe + UnwindSafe>;

/// A message sent to a canister that trigger execution of a task on the canister's execution thread
/// based on the type of the message.
pub enum Message {
    /// A custom function that you want to be executed in the canister's execution thread.
    CustomTask {
        /// The request id of this incoming message.
        request_id: IncomingRequestId,
        /// the task handler that should be executed in the canister's execution thread.
        task: TaskFn,
        /// The env to use for this custom execution.
        env: Env,
    },
    /// A top-level request to the canister.
    Request {
        /// The request id of the incoming message. Must be None if the reply_to is set.
        request_id: IncomingRequestId,
        /// The env to use during the execution of this task.
        env: Env,
    },
    // Either a reply_callback or reject_callbacks.
    Reply {
        /// Which request is this reply for.
        reply_to: OutgoingRequestId,
        /// The env to use for this, assert:
        ///     env.entry_mode == ReplyCallback
        ///     env.entry_mode == RejectCallback
        env: Env,
    },
}

/// A call that has made to another canister.
#[derive(Debug)]
pub struct CanisterCall {
    pub sender: Principal,
    pub request_id: RequestId,
    pub callee: Principal,
    pub method: String,
    pub payment: u128,
    pub arg: Vec<u8>,
}

impl From<CanisterCall> for Message {
    fn from(call: CanisterCall) -> Self {
        Message::Request {
            request_id: call.request_id,
            env: Env::default()
                .with_entry_mode(EntryMode::Update)
                .with_sender(call.sender)
                .with_method_name(call.method)
                .with_cycles_available(call.payment)
                .with_raw_args(call.arg),
        }
    }
}

impl Default for Env {
    fn default() -> Self {
        Env {
            balance: 100_000_000_000_000,
            entry_mode: EntryMode::CustomTask,
            sender: Principal::anonymous(),
            method_name: None,
            cycles_available: 0,
            cycles_refunded: 0,
            args: CANDID_EMPTY_ARG.to_vec(),
            rejection_code: RejectionCode::NoError,
            rejection_message: String::new(),
            time: now(),
        }
    }
}

impl Env {
    /// Create a new env for an update call.
    pub fn update<S: Into<String>>(method_name: S) -> Self {
        Self::default()
            .with_entry_mode(EntryMode::Update)
            .with_method_name(method_name)
    }

    /// Create a new env for a query call.
    pub fn query<S: Into<String>>(method_name: S) -> Self {
        Self::default()
            .with_entry_mode(EntryMode::Query)
            .with_method_name(method_name)
    }

    /// Create a new env for a call to the init function.
    pub fn init() -> Self {
        Self::default().with_entry_mode(EntryMode::Init)
    }

    /// Create a new env for a call to the pre_upgrade function.
    pub fn pre_upgrade() -> Self {
        Self::default().with_entry_mode(EntryMode::PreUpgrade)
    }

    /// Create a new env for a call to the post_upgrade function.
    pub fn post_upgrade() -> Self {
        Self::default().with_entry_mode(EntryMode::PostUpgrade)
    }

    /// Create a new env for a call to the heartbeat function.
    pub fn heartbeat() -> Self {
        Self::default().with_entry_mode(EntryMode::Heartbeat)
    }

    /// Determines the canister's cycle balance for this call.
    pub fn with_balance(mut self, balance: u128) -> Self {
        self.balance = balance;
        self
    }

    /// Use the provided time for this env.
    pub fn with_time(mut self, time: u64) -> Self {
        self.time = time;
        self
    }

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

    /// Provide the current env with the given amount of cycles to execute.
    pub fn with_cycles_available(mut self, cycles: u128) -> Self {
        self.cycles_available = cycles;
        self
    }

    /// Provide the current env with the given amount of refunded cycles, only applicable
    /// if this is reply/reject callback.
    pub fn with_cycles_refunded(mut self, cycles: u128) -> Self {
        self.cycles_refunded = cycles;
        self
    }

    /// The arguments in this environment, in a reply mode this is the data returned to the
    /// canister.
    pub fn with_raw_args<A: Into<Vec<u8>>>(mut self, argument: A) -> Self {
        self.args = argument.into();
        self
    }

    /// Encode the provided tuple using candid and use it as arguments during this execution.
    pub fn with_args<T: ArgumentEncoder>(mut self, arguments: T) -> Self {
        self.args = encode_args(arguments).unwrap();
        self
    }

    /// Shorthand for `with_args((argument, ))` to pass tuples with only one element to the call.
    pub fn with_arg<T: CandidType>(mut self, argument: T) -> Self {
        self.args = encode_one(argument).unwrap();
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

    /// Returns the second possible name of this entry point.
    pub fn get_possible_entry_point_name(&self) -> String {
        match &self.entry_mode {
            EntryMode::Update => {
                format!(
                    "canister_query {}",
                    self.method_name.as_ref().unwrap_or(&String::new())
                )
            }
            EntryMode::Query => format!(
                "canister_update {}",
                self.method_name.as_ref().unwrap_or(&String::new())
            ),
            _ => self.get_entry_point_name(),
        }
    }
}

fn now() -> u64 {
    let now = SystemTime::now();
    let unix = now
        .duration_since(UNIX_EPOCH)
        .expect("ic-kit-runtime: could not retrieve unix time.");
    unix.as_nanos() as u64
}
