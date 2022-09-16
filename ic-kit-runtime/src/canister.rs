use crate::call::CallReply;
use crate::stable::{HeapStableMemory, StableMemoryBackend};
use crate::types::*;
use futures::executor::block_on;
use ic_kit_sys::ic0;
use ic_kit_sys::ic0::runtime;
use ic_kit_sys::ic0::runtime::Ic0CallHandlerProxy;
use ic_kit_sys::types::RejectionCode;
use ic_types::Principal;
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::panic::catch_unwind;
use std::thread::JoinHandle;
use thread_local_panic_hook::set_hook;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

const MAX_CYCLES_PER_RESPONSE: u128 = 12;

/// A canister that is being executed.
pub struct Canister {
    /// The id of the canister.
    canister_id: Principal,
    /// Maps the name of each of exported methods to the task function.
    symbol_table: HashMap<String, fn()>,
    /// The data reply that is being built for the current message. An interesting thing about the
    /// IC that I did not expect: The reply data is not preserved throughout the async context.
    /// And the reply is the first call to msg_reply that is inside a non-trapping task.
    msg_reply_data: Vec<u8>,
    /// Map each incoming request to its response channel, if it is None, it means the
    /// message has already been responded to.
    msg_reply_senders: HashMap<IncomingRequestId, oneshot::Sender<CallReply>>,
    /// The reply for the current call that can be sent via msg_reply_senders channel once the
    /// current message has been processed without trapping.
    msg_reply: Option<CallReply>,
    /// The amount of available cycles for each incoming request. This is only used
    /// for recovering self.env state for reply callbacks.
    cycles_available_store: HashMap<IncomingRequestId, u128>,
    /// Amount of cycles accept during this message process.
    cycles_accepted: u128,
    /// Pending outgoing requests that have not been resolved yet. This is used so we know when
    /// an incoming request is finally finished so we can send the last trapping message as the
    /// response.
    pending_outgoing_requests: HashMap<IncomingRequestId, HashSet<OutgoingRequestId>>,
    /// Map each of the out going requests done by this canister to the callbacks for that
    /// call.
    outgoing_calls: HashMap<OutgoingRequestId, RequestCallbacks>,
    /// The canister execution environment.
    env: Env,
    /// The stable storage backend for this canister.
    stable: Box<dyn StableMemoryBackend + Send>,
    /// The request id of the current incoming message.
    request_id: Option<IncomingRequestId>,
    /// The calls that are finalized and should be sent after this entry point's successful
    /// execution.
    call_queue: Vec<(Principal, String, RequestCallbacks, u128, Vec<u8>)>,
    /// The current call under construction, once call_perform is called, this will go into
    /// the call_queue to be performed later on.
    pending_call: Option<(Principal, String, RequestCallbacks, u128, Vec<u8>)>,
    /// The thread in which the canister is being executed at.
    _execution_thread_handle: JoinHandle<()>,
    /// The communication channel to send tasks to the execution thread.
    task_tx: Sender<TaskFn>,
    /// Emits when the task we just sent has returned.
    task_completion_rx: Receiver<Completion>,
    /// To send the response to the calls.
    reply_tx: Sender<runtime::Response>,
    /// The channel that we use to get the requests from the execution thread.
    request_rx: Receiver<runtime::Request>,
}

#[derive(Debug)]
enum Completion {
    Ok,
    Panicked(String),
}

/// Any of the reply, reject or clean up callbacks.
/// (callback_fun, callback_env)
///
/// The callback_fun can be set to -1 for one-way calls.
type Callback = (isize, isize);

/// The callbacks
struct RequestCallbacks {
    /// The original top-level message which caused this inter-canister call, this is used so
    /// for example when `ic0::msg_reply` is called, we know which call to respond to.
    message_id: IncomingRequestId,
    /// The reply callback that must be called for a reply.
    reply: Callback,
    /// The reject callback that must be called for a reject.
    reject: Callback,
    /// An optional cleanup callback.
    cleanup: Option<Callback>,
}

/// A method exported by the canister.
pub trait CanisterMethod {
    /// The export name of this method, this is the name that the method is
    /// exported by in the WASM binary, examples could be:
    /// - `canister_init`
    /// - `canister_update increment`
    /// - `canister_pre_upgrade`
    ///
    /// See:
    /// https://internetcomputer.org/docs/current/references/ic-interface-spec/#entry-points
    const EXPORT_NAME: &'static str;

    /// The method which is exported by the canister in the WASM, since the entry points
    /// should have a type `() -> ()`, we wrap the canister methods in a function in which
    /// we perform the serialization/deserialization of arguments/responses, using the runtime
    /// primitives.
    fn exported_method();
}

impl Canister {
    /// Create a new instance of this canister with the given id.
    pub fn new<T: Into<Principal>>(canister_id: T) -> Self {
        let (request_tx, request_rx) = mpsc::channel(8);
        let (reply_tx, reply_rx) = mpsc::channel(8);
        let (task_tx, mut task_rx) = mpsc::channel::<TaskFn>(8);
        let (task_completion_tx, task_completion_rx) = mpsc::channel(8);

        let execution_thread_handle = std::thread::spawn(move || {
            // Register the ic-kit-sys handler for current thread, this will make ic-kit-sys to
            // forward all of the system calls done in the current thread to the provided channel
            // and use the rx channel for waiting on responses.
            let handle = runtime::RuntimeHandle::new(reply_rx, request_tx);
            ic0::register_handler(handle);

            // set the custom panic hook for this thread, this will give us:
            // - No message such as "thread panic during test" in the terminal.
            // - TODO: Capture the panic location.
            // let panic_hook_tx = task_completion_tx.clone();
            set_hook(Box::new(|_| {}));

            while let Some(task) = block_on(task_rx.recv()) {
                let c = if let Err(payload) = catch_unwind(|| {
                    task();
                }) {
                    Completion::Panicked(downcast_panic_payload(&payload))
                } else {
                    Completion::Ok
                };

                // In case we panic the hook might have already sent the proper panic message,
                // and we may be double sending this signal here, but this is okay since,
                // process_message always makes sure there is no pending signals in this channel
                // before sending a new task.
                block_on(task_completion_tx.send(c))
                    .expect("ic-kit-runtime: Execution thread could not send task-completion signal to the main thread.");
            }
        });

        Self {
            canister_id: canister_id.into(),
            symbol_table: HashMap::new(),
            msg_reply_data: Vec::new(),
            msg_reply_senders: HashMap::new(),
            msg_reply: None,
            cycles_available_store: HashMap::new(),
            cycles_accepted: 0,
            pending_outgoing_requests: HashMap::new(),
            outgoing_calls: HashMap::new(),
            env: Env::default(),
            stable: Box::new(HeapStableMemory::default()),
            request_id: None,
            call_queue: Vec::with_capacity(8),
            pending_call: None,
            _execution_thread_handle: execution_thread_handle,
            task_tx,
            task_completion_rx,
            reply_tx,
            request_rx,
        }
    }

    /// Return the canister ID.
    pub fn id(&self) -> Principal {
        self.canister_id
    }

    /// Provide the canister with the definition of the given method.
    pub fn with_method<M: CanisterMethod + 'static>(mut self) -> Self {
        let method_name = String::from(M::EXPORT_NAME);
        let task_fn = M::exported_method;

        if self.symbol_table.contains_key(&method_name) {
            panic!("The canister already has a '{}' method.", method_name);
        }

        self.symbol_table.insert(method_name, task_fn);
        self
    }

    /// Provide the canister with this stable storage backend.
    pub fn with_stable(mut self, stable: Box<dyn StableMemoryBackend + Send>) -> Self {
        self.stable = stable;
        self
    }

    pub async fn process_message(
        &mut self,
        message: CanisterMessage,
        reply_sender: Option<oneshot::Sender<CallReply>>,
    ) -> Vec<CanisterCall> {
        // Force reset the state.
        self.discard_pending_call();
        self.discard_call_queue();
        self.request_id = None;
        self.cycles_accepted = 0;

        // Assign the request_id for this message.
        let (request_id, env, task) = match message {
            CanisterMessage::CustomTask {
                request_id,
                env,
                task,
            } => {
                assert!(
                    reply_sender.is_some(),
                    "A request must provide a response channel."
                );

                assert!(
                    env.entry_mode != EntryMode::ReplyCallback
                        && env.entry_mode != EntryMode::RejectCallback
                );

                (request_id, env, Some(task))
            }
            CanisterMessage::Request { request_id, env } => {
                assert!(
                    reply_sender.is_some(),
                    "A request must provide a response channel."
                );

                assert!(
                    env.entry_mode != EntryMode::ReplyCallback
                        && env.entry_mode != EntryMode::RejectCallback
                        && env.entry_mode != EntryMode::CleanupCallback
                        && env.entry_mode != EntryMode::CustomTask
                );

                let entry_point_name = env.get_entry_point_name();
                let task = self
                    .symbol_table
                    .get(&entry_point_name)
                    .or_else(|| self.symbol_table.get(&env.get_possible_entry_point_name()))
                    .map(|f| {
                        let f = f.clone();
                        Box::new(move || {
                            f();
                        }) as TaskFn
                    });

                (request_id, env, task)
            }
            CanisterMessage::Reply { reply_to, env } => {
                let callbacks = self.outgoing_calls.remove(&reply_to).expect(
                    "ic-kit-runtime: No outgoing message with the given id on this canister.",
                );

                let id = callbacks.message_id;
                let _clean_callbacks = callbacks.cleanup;

                assert!(
                    env.entry_mode == EntryMode::ReplyCallback
                        || env.entry_mode == EntryMode::RejectCallback
                );

                let set = self.pending_outgoing_requests.get_mut(&id).unwrap();
                set.remove(&reply_to);

                if set.is_empty() {
                    self.pending_outgoing_requests.remove(&id);
                }

                let (fun, fun_env) = match env.entry_mode {
                    EntryMode::ReplyCallback => callbacks.reply,
                    EntryMode::RejectCallback => callbacks.reject,
                    _ => unreachable!(),
                };

                let task = Box::new(move || unsafe {
                    // -1 is used by a one-way call.
                    if fun != -1 {
                        let fun = std::mem::transmute::<isize, fn(isize)>(fun);
                        fun(fun_env);
                    }
                }) as TaskFn;

                (id, env, Some(task))
            }
        };

        if task.is_none() {
            let chan = reply_sender.unwrap();

            let reply = CallReply::Reject {
                rejection_code: RejectionCode::DestinationInvalid,
                rejection_message: format!(
                    "Canister does not have a '{}' method.",
                    env.method_name.unwrap_or_default()
                ),
                cycles_refunded: env.cycles_available,
            };

            chan.send(reply)
                .expect("ic-kit-runtime: Could not send the message reply.");

            return Vec::new();
        }

        self.request_id = Some(request_id);
        self.env = env;
        self.env.cycles_available = *self
            .cycles_available_store
            .entry(request_id)
            .or_insert(self.env.cycles_available);
        self.env.balance += self.env.cycles_refunded;

        if let Some(sender) = reply_sender {
            self.msg_reply_senders
                .insert(self.request_id.unwrap(), sender);
        }

        let completion = self.perform(task.unwrap()).await;

        match completion {
            Completion::Panicked(m) => {
                // We panicked, so we don't want to send any of the outgoing messages.
                self.discard_call_queue();
                // return the cycles available in this call.
                self.env.cycles_available += self.cycles_accepted;
                self.cycles_accepted = 0;
                self.cycles_available_store
                    .insert(self.request_id.unwrap(), self.env.cycles_available);
                self.maybe_final_reply(Some(m), self.env.cycles_available);
            }
            Completion::Ok => {
                if let Some(reply) = self.msg_reply.take() {
                    let chan = self
                        .msg_reply_senders
                        .remove(&self.request_id.unwrap())
                        .expect("ic-kit-runtime: Response channel not found for request.");

                    chan.send(reply)
                        .expect("ic-kit-runtime: Could not send the message reply.")
                }

                self.maybe_final_reply(None, self.env.cycles_available);
            }
        };

        let queue = std::mem::replace(&mut self.call_queue, Vec::new());
        let mut tmp = Vec::<CanisterCall>::with_capacity(queue.len());
        for (callee, method, cb, payment, arg) in queue {
            let request_id = RequestId::new();

            // Insert the pending request id for the current call.
            self.pending_outgoing_requests
                .entry(self.request_id.unwrap())
                .or_default()
                .insert(request_id);

            // Store the callbacks to wake up the caller.
            self.outgoing_calls.insert(request_id, cb);

            tmp.push(CanisterCall {
                sender: self.id(),
                request_id,
                callee,
                method,
                payment,
                arg,
            });
        }

        tmp
    }

    /// Execute the given task in the execution thread and return the completion status.
    async fn perform(&mut self, task: TaskFn) -> Completion {
        // make sure we clean the task_returned receiver. since we may have sent more than one
        // completion signal from previous task.
        while self.task_completion_rx.try_recv().is_ok() {}
        while self.request_rx.try_recv().is_ok() {}

        self.task_tx.send(task).await.unwrap_or_else(|_| {
            panic!("ic-kit-runtime: Could not send the task to the execution thread.")
        });

        let completion: Completion = loop {
            select! {
                Some(c) = self.task_completion_rx.recv() => {
                    // We got the completion signal, which means the task finished execution.
                    break c;
                },
                Some(req) = self.request_rx.recv() => {
                    let res = req.proxy(self);
                    self.reply_tx
                        .send(res)
                        .await
                        .expect("ic-kit-runtime: Could not send the system API call's response to the execution thread.");
                }
            }
        };

        // Discard the pending call regardless of the completion status.
        self.discard_pending_call();

        completion
    }

    /// Send the final reply for the current call if none has already been sent.
    fn maybe_final_reply(&mut self, trap_message: Option<String>, cycles: u128) {
        let id = match self.request_id {
            Some(id) => id,
            None => return,
        };

        // There are still pending outgoing calls we have to wait for them to finish.
        if self.pending_outgoing_requests.contains_key(&id) || !self.call_queue.is_empty() {
            return;
        }

        let chan = match self.msg_reply_senders.remove(&id) {
            Some(c) => c,
            None => return,
        };

        self.cycles_available_store.remove(&id);

        chan.send(CallReply::Reject {
            rejection_code: RejectionCode::CanisterError,
            rejection_message: trap_message
                .unwrap_or_else(|| "Canister did not reply to the call".to_string()),
            cycles_refunded: cycles,
        })
        .expect("ic-kit-runtime: Could not send the message reply.")
    }

    fn discard_pending_call(&mut self) {
        if let Some(pending_call) = self.pending_call.take() {
            self.env.balance += MAX_CYCLES_PER_RESPONSE + pending_call.3;
        }
    }

    fn discard_call_queue(&mut self) {
        while let Some(pending_call) = self.call_queue.pop() {
            self.env.balance += MAX_CYCLES_PER_RESPONSE + pending_call.3;
        }
    }
}

impl Ic0CallHandlerProxy for Canister {
    fn msg_arg_data_size(&mut self) -> Result<isize, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Init
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::ReplyCallback
            | EntryMode::InspectMessage => Ok(self.env.args.len() as isize),
            _ => Err(format!(
                "msg_arg_data_size can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_arg_data_copy(&mut self, dst: isize, offset: isize, size: isize) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Init
            | EntryMode::PostUpgrade
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::ReplyCallback
            | EntryMode::InspectMessage => {
                let data = self.env.args.as_slice();
                copy_to_canister(dst, offset, size, data)?;
                Ok(())
            }
            _ => Err(format!(
                "msg_arg_data_copy can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_caller_size(&mut self) -> Result<isize, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Init
            | EntryMode::PostUpgrade
            | EntryMode::PreUpgrade
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::InspectMessage => Ok(self.env.sender.as_slice().len() as isize),
            _ => Err(format!(
                "msg_caller_size can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_caller_copy(&mut self, dst: isize, offset: isize, size: isize) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Init
            | EntryMode::PostUpgrade
            | EntryMode::PreUpgrade
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::InspectMessage => {
                let data = self.env.sender.as_slice();
                copy_to_canister(dst, offset, size, data)?;
                Ok(())
            }
            _ => Err(format!(
                "msg_caller_copy can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_reject_code(&mut self) -> Result<i32, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::ReplyCallback | EntryMode::RejectCallback => {
                Ok(self.env.rejection_code as i32)
            }
            _ => Err(format!(
                "msg_reject_code can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_reject_msg_size(&mut self) -> Result<isize, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::RejectCallback => {
                Ok(self.env.rejection_message.len() as isize)
            }
            _ => Err(format!(
                "msg_reject_msg_size can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_reject_msg_copy(
        &mut self,
        dst: isize,
        offset: isize,
        size: isize,
    ) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::RejectCallback => {
                let data = self.env.rejection_message.as_bytes();
                copy_to_canister(dst, offset, size, data)?;
                Ok(())
            }
            _ => Err(format!(
                "msg_reject_msg_copy can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_reply_data_append(&mut self, src: isize, size: isize) -> Result<(), String> {
        let message_id = match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => {
                // this should always be present when processing a call.
                self.request_id
                    .expect("ic-kit: Unexpected canister state, request_id not set.")
            }
            _ => {
                return Err(format!(
                    "msg_reply_data_append can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        if !self.msg_reply_senders.contains_key(&message_id) {
            return Err(
                "msg_reply_data_append may only be invoked before canister responses.".to_string(),
            );
        }

        self.msg_reply_data
            .extend_from_slice(copy_from_canister(src, size));

        Ok(())
    }

    fn msg_reply(&mut self) -> Result<(), String> {
        let message_id = match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => {
                // this should always be present when processing a call.
                self.request_id
                    .expect("ic-kit: Unexpected canister state, request_id not set.")
            }
            _ => {
                return Err(format!(
                    "msg_reply can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        // We have either replied to this message in the current task execution, so the msg_reply
        // contains data, or we have done this in previous task execution for this incoming message
        // so the msg_reply_sender channel is no longer available.
        if self.msg_reply.is_some() || !self.msg_reply_senders.contains_key(&message_id) {
            return Err("Current call is already replied to.".to_string());
        }

        let data = self.msg_reply_data.clone();
        self.msg_reply_data.clear();
        let cycles_refunded = self.env.cycles_available;
        self.env.cycles_available = 0;
        self.msg_reply = Some(CallReply::Reply {
            data,
            cycles_refunded,
        });

        Ok(())
    }

    fn msg_reject(&mut self, src: isize, size: isize) -> Result<(), String> {
        let message_id = match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::Query
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => {
                // this should always be present when processing a call.
                self.request_id
                    .expect("ic-kit: Unexpected canister state, request_id not set.")
            }
            _ => {
                return Err(format!(
                    "msg_reject can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        self.msg_reply_data.clear();

        // see: msg_reply
        if self.msg_reply.is_some() || !self.msg_reply_senders.contains_key(&message_id) {
            return Err("Current call is already replied to.".to_string());
        }

        let cycles_refunded = self.env.cycles_available;
        let rejection_message = String::from_utf8_lossy(copy_from_canister(src, size)).into();
        self.env.cycles_available = 0;
        self.msg_reply = Some(CallReply::Reject {
            rejection_code: RejectionCode::CanisterReject,
            rejection_message,
            cycles_refunded,
        });

        Ok(())
    }

    fn msg_cycles_available(&mut self) -> Result<i64, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => {
                if self.env.cycles_available > (u64::MAX as u128) {
                    return Err("available cycles does not fit in u64".to_string());
                }

                Ok(self.env.cycles_available as u64 as i64)
            }
            _ => Err(format!(
                "msg_cycles_available can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_cycles_available128(&mut self, dst: isize) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => {
                let data = self.env.cycles_available.to_le_bytes();
                copy_to_canister(dst, 0, 16, &data)?;
                Ok(())
            }
            _ => Err(format!(
                "msg_cycles_available128 can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_cycles_refunded(&mut self) -> Result<i64, String> {
        match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::ReplyCallback | EntryMode::RejectCallback => {
                if self.env.cycles_refunded > (u64::MAX as u128) {
                    return Err("refunded cycles does not fit in u64".to_string());
                }

                Ok(self.env.cycles_refunded as u64 as i64)
            }
            _ => Err(format!(
                "msg_cycles_refunded can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_cycles_refunded128(&mut self, dst: isize) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::ReplyCallback | EntryMode::RejectCallback => {
                let data = self.env.cycles_refunded.to_le_bytes();
                copy_to_canister(dst, 0, 16, &data)?;
                Ok(())
            }
            _ => Err(format!(
                "msg_cycles_refunded128 can not be called from '{}'",
                self.env.get_entry_point_name()
            )),
        }
    }

    fn msg_cycles_accept(&mut self, max_amount: i64) -> Result<i64, String> {
        let message_id = match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => self
                .request_id
                .expect("ic-kit: Unexpected canister state, request_id not set."),
            _ => {
                return Err(format!(
                    "msg_cycles_accept can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        let amount = self.env.cycles_available.min(max_amount as u128);
        self.env.cycles_available -= amount;
        self.cycles_accepted += amount;
        self.cycles_available_store
            .insert(message_id, self.env.cycles_available);

        Ok(amount as i64)
    }

    fn msg_cycles_accept128(
        &mut self,
        max_amount_high: i64,
        max_amount_low: i64,
        dst: isize,
    ) -> Result<(), String> {
        let message_id = match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback => self
                .request_id
                .expect("ic-kit: Unexpected canister state, request_id not set."),
            _ => {
                return Err(format!(
                    "msg_cycles_accept128 can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        let high = max_amount_high as u128;
        let low = max_amount_low as u128;
        let max_amount = high << 64 + low;
        let amount = self.env.cycles_available.min(max_amount);
        self.env.cycles_available -= amount;
        self.cycles_accepted += amount;
        self.cycles_available_store
            .insert(message_id, self.env.cycles_available);
        copy_to_canister(dst, 0, 16, &amount.to_le_bytes())?;

        Ok(())
    }

    fn canister_self_size(&mut self) -> Result<isize, String> {
        Ok(self.canister_id.as_slice().len() as isize)
    }

    fn canister_self_copy(&mut self, dst: isize, offset: isize, size: isize) -> Result<(), String> {
        let data = self.canister_id.as_slice();
        copy_to_canister(dst, offset, size, data)?;
        Ok(())
    }

    fn canister_cycle_balance(&mut self) -> Result<i64, String> {
        let balance = self.env.balance + self.cycles_accepted;

        if balance > (u64::MAX as u128) {
            return Err("refunded cycles does not fit in u64".to_string());
        }

        Ok(balance as u64 as i64)
    }

    fn canister_cycle_balance128(&mut self, dst: isize) -> Result<(), String> {
        let balance = self.env.balance + self.cycles_accepted;
        let data = balance.to_le_bytes();
        copy_to_canister(dst, 0, 16, &data)?;
        Ok(())
    }

    fn canister_status(&mut self) -> Result<i32, String> {
        // TODO(qti3e) support stopping canisters.
        Ok(1)
    }

    fn msg_method_name_size(&mut self) -> Result<isize, String> {
        let method_name = match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::InspectMessage => self
                .env
                .method_name
                .as_ref()
                .expect("ic-kit-runtime: Method name is not set.")
                .as_bytes(),
            _ => {
                return Err(format!(
                    "msg_method_name_size can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        Ok(method_name.len() as isize)
    }

    fn msg_method_name_copy(
        &mut self,
        dst: isize,
        offset: isize,
        size: isize,
    ) -> Result<(), String> {
        let method_name = match self.env.entry_mode {
            EntryMode::CustomTask | EntryMode::InspectMessage => self
                .env
                .method_name
                .as_ref()
                .expect("ic-kit-runtime: Method name is not set.")
                .as_bytes(),
            _ => {
                return Err(format!(
                    "msg_method_name_copy can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        };

        copy_to_canister(dst, offset, size, method_name)?;
        Ok(())
    }

    fn accept_message(&mut self) -> Result<(), String> {
        // TODO(qti3e) Hmm.. this has room for some thoughts.
        todo!()
    }

    fn call_new(
        &mut self,
        callee_src: isize,
        callee_size: isize,
        name_src: isize,
        name_size: isize,
        reply_fun: isize,
        reply_env: isize,
        reject_fun: isize,
        reject_env: isize,
    ) -> Result<(), String> {
        match self.env.entry_mode {
            EntryMode::CustomTask
            | EntryMode::Update
            | EntryMode::ReplyCallback
            | EntryMode::RejectCallback
            | EntryMode::Heartbeat => {}
            _ => {
                return Err(format!(
                    "call_new can not be called from '{}'",
                    self.env.get_entry_point_name()
                ))
            }
        }

        self.discard_pending_call();

        if self.env.balance < MAX_CYCLES_PER_RESPONSE {
            return Err("Insufficient cycles balance to process canister response.".into());
        }

        self.env.balance -= MAX_CYCLES_PER_RESPONSE;

        let callee_bytes = copy_from_canister(callee_src, callee_size);
        let name_bytes = copy_from_canister(name_src, name_size);
        let callee = Principal::from_slice(callee_bytes);
        let name = String::from_utf8_lossy(name_bytes).to_string();
        let callbacks = RequestCallbacks {
            message_id: self
                .request_id
                .expect("ic-kit-runtime: Request ID not set."),
            reply: (reply_fun, reply_env),
            reject: (reject_fun, reject_env),
            cleanup: None,
        };

        self.pending_call = Some((callee, name, callbacks, 0, Vec::new()));

        Ok(())
    }

    fn call_on_cleanup(&mut self, fun: isize, env: isize) -> Result<(), String> {
        if self.pending_call.is_none() {
            return Err(format!(
                "call_on_cleanup cannot be called when there is no pending call."
            ));
        }

        let cleanup = &mut self.pending_call.as_mut().unwrap().2.cleanup;

        if cleanup.is_some() {
            return Err(format!("call_on_cleanup cannot be invoked more than once."));
        }

        *cleanup = Some((fun, env));

        Ok(())
    }

    fn call_data_append(&mut self, src: isize, size: isize) -> Result<(), String> {
        if self.pending_call.is_none() {
            return Err(format!(
                "call_data_append cannot be called when there is no pending call."
            ));
        }

        let args = &mut self.pending_call.as_mut().unwrap().4;
        let bytes = copy_from_canister(src, size);
        args.extend_from_slice(bytes);

        Ok(())
    }

    fn call_cycles_add(&mut self, amount: i64) -> Result<(), String> {
        if self.pending_call.is_none() {
            return Err(format!(
                "call_cycles_add cannot be called when there is no pending call."
            ));
        }

        let amount = amount as u128;

        if self.env.balance < amount {
            return Err(format!("Insufficient cycles balance."));
        }

        self.env.balance -= amount;
        self.pending_call.as_mut().unwrap().3 += amount;

        Ok(())
    }

    fn call_cycles_add128(&mut self, amount_high: i64, amount_low: i64) -> Result<(), String> {
        if self.pending_call.is_none() {
            return Err(format!(
                "call_cycles_add128 cannot be called when there is no pending call."
            ));
        }

        let high = amount_high as u128;
        let low = amount_low as u128;
        let amount = high << 64 + low;

        if self.env.balance < amount {
            return Err(format!("Insufficient cycles balance."));
        }

        self.env.balance -= amount;
        self.pending_call.as_mut().unwrap().3 += amount;

        Ok(())
    }

    fn call_perform(&mut self) -> Result<i32, String> {
        if self.pending_call.is_none() {
            return Err(format!(
                "call_cycles_add128 cannot be called when there is no pending call."
            ));
        }

        // TODO(qti3e) Implement the freezing threshold + system ability to perform call.
        // For now all of the calls go through.

        self.call_queue.push(self.pending_call.take().unwrap());
        Ok(0)
    }

    fn stable_size(&mut self) -> Result<i32, String> {
        match self.stable.stable_size().try_into() {
            Ok(size) => Ok(size),
            Err(_) => Err("Invalid stable size".into()),
        }
    }

    fn stable_grow(&mut self, new_pages: i32) -> Result<i32, String> {
        let size = self.stable.stable_size() as i32;
        let max_size = i32::max_value();

        if size + new_pages > max_size {
            Ok(-1)
        } else {
            Ok(self.stable.stable_grow(new_pages as u64) as i32)
        }
    }

    fn stable_write(&mut self, _offset: i32, _src: isize, _size: isize) -> Result<(), String> {
        self.stable
            .stable_write(_offset as u64, copy_from_canister(_src, _size));

        Ok(())
    }

    fn stable_read(&mut self, dst: isize, offset: i32, size: isize) -> Result<(), String> {
        let mut buf = vec![0u8; size as usize];
        self.stable.stable_read(offset as u64, &mut buf);
        copy_to_canister(dst, offset as isize, size, &buf)?;
        Ok(())
    }

    fn stable64_size(&mut self) -> Result<i64, String> {
        Ok(self.stable.stable_size() as i64)
    }

    fn stable64_grow(&mut self, new_pages: i64) -> Result<i64, String> {
        Ok(self.stable.stable_grow(new_pages as u64) as i64)
    }

    fn stable64_write(&mut self, offset: i64, src: i64, size: i64) -> Result<(), String> {
        Ok(self.stable.stable_write(
            offset as u64,
            copy_from_canister(src as isize, size as isize),
        ))
    }

    fn stable64_read(&mut self, dst: i64, offset: i64, size: i64) -> Result<(), String> {
        let mut buf = vec![0u8; size as usize];
        self.stable.stable_read(offset as u64, &mut buf);
        copy_to_canister(dst as isize, offset as isize, size as isize, &buf)?;
        Ok(())
    }

    fn certified_data_set(&mut self, _src: isize, _size: isize) -> Result<(), String> {
        todo!()
    }

    fn data_certificate_present(&mut self) -> Result<i32, String> {
        todo!()
    }

    fn data_certificate_size(&mut self) -> Result<isize, String> {
        todo!()
    }

    fn data_certificate_copy(
        &mut self,
        _dst: isize,
        _offset: isize,
        _size: isize,
    ) -> Result<(), String> {
        todo!()
    }

    fn time(&mut self) -> Result<i64, String> {
        Ok(self.env.time as i64)
    }

    fn performance_counter(&mut self, _counter_type: i32) -> Result<i64, String> {
        todo!()
    }

    fn debug_print(&mut self, src: isize, size: isize) -> Result<(), String> {
        let bytes = copy_from_canister(src, size);
        let message = String::from_utf8_lossy(bytes).to_string();
        println!("canister: {}", message);
        Ok(())
    }

    fn trap(&mut self, src: isize, size: isize) -> Result<(), String> {
        let bytes = copy_from_canister(src, size);
        let message = String::from_utf8_lossy(bytes).to_string();
        Err(format!("Canister trapped: '{}'", message))
    }
}

fn copy_to_canister(dst: isize, offset: isize, size: isize, data: &[u8]) -> Result<(), String> {
    let dst = dst as usize;
    let offset = offset as usize;
    let size = size as usize;

    if offset + size > data.len() {
        return Err("Out of bound read.".into());
    }

    let slice = unsafe { std::slice::from_raw_parts_mut(dst as *mut u8, size) };
    slice.copy_from_slice(&data[offset..offset + size]);
    Ok(())
}

fn copy_from_canister<'a>(src: isize, size: isize) -> &'a [u8] {
    let src = src as usize;
    let size = size as usize;

    unsafe { std::slice::from_raw_parts(src as *const u8, size) }
}

fn downcast_panic_payload(payload: &Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&'static str>()
        .cloned()
        .map(String::from)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| String::from("Box<Any>"))
}
