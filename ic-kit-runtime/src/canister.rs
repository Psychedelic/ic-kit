use crate::canister_id::CanisterId;
use futures::executor::block_on;
use ic_kit_sys::ic0;
use ic_kit_sys::ic0::runtime;
use ic_kit_sys::ic0::Ic0CallHandler;
use ic_types::Principal;
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::panic::set_hook;
use std::sync::atomic::AtomicU64;
use std::thread::JoinHandle;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::oneshot;

/// A canister that is being executed.
pub struct Canister {
    /// The id of the canister.
    canister_id: Vec<u8>,
    /// The canister balance.
    balance: u128,
    /// The request we're currently processing.
    processing: Option<Request>,
    /// The thread in which the canister is being executed at.
    execution_thread_handle: JoinHandle<()>,
    /// The communication channel to send tasks to the execution thread.
    task_tx: Sender<Box<dyn Fn() + Send>>,
    /// Emits when the task we just sent has returned.
    task_returned_rx: Receiver<()>,
    /// To send the response to the calls.
    reply_tx: Sender<runtime::Response>,
    /// The channel that we use to get the requests from the execution thread.
    request_rx: Receiver<runtime::Request>,
    /// Maps the name of each of exported methods to the task function.
    symbol_table: HashMap<String, Box<dyn Fn() + Send>>,
}

/// Any of the reply, reject or clean up callbacks.
/// (callback_fun, callback_env)
type Callback = (isize, isize);

/// The callbacks
struct RequestCallbacks {
    /// The original top-level message which caused this inter-canister call, this is used so
    /// for example when `ic0::msg_reply` is called, we know which call to respond to.
    message_id: RequestId,
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

pub enum Request {
    Init {
        sender: Vec<u8>,
        arg: Vec<u8>,
    },
    Update {
        sender: Vec<u8>,
        arg: Vec<u8>,
        cycles: u128,
    },
    Query {
        sender: Vec<u8>,
        arg: Vec<u8>,
    },
    MessageResponse {
        /// ID of the outgoing request to the other canister.
        request_id: RequestId,
        /// Data returned by that canister.
        data: Vec<u8>,
        /// Number of cycles refunded from the other canister.
        cycles_refunded: u128,
    },
    MessageReject {
        request_id: RequestId,
        rejection_code: RejectionCode,
        message: Vec<u8>,
        /// Number of refunded cycles, always equals to
        cycles_refunded: u128,
    },
    Heartbeat,
    PostUpgrade,
    PreUpgrade,
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

impl Canister {
    pub fn new(canister_id: CanisterId) -> Self {
        let (request_tx, request_rx) = mpsc::channel(8);
        let (reply_tx, reply_rx) = mpsc::channel(8);
        let (task_tx, mut task_rx) = mpsc::channel::<Box<dyn Fn() + Send>>(8);
        let (mut task_returned_tx, task_returned_rx) = mpsc::channel(8);

        let thread = std::thread::spawn(move || {
            // Register the ic-kit-sys handler for current thread, this will make ic-kit-sys to
            // forward all of the system calls done in the current thread to the provided channel
            // and use the rx channel for waiting on responses.
            let handle = runtime::RuntimeHandle::new(reply_rx, request_tx);
            ic0::register_handler(handle);

            // set the custom panic hook, this will give us:
            // 1. No message such as "thread panic during test" in the terminal.
            // 2. Allow us to notify the main thread that we panicked.
            // 3. Also allows us to signal the task runner that the task has returned, so it can
            //    stop waiting for requests made by us.
            let task_panicked_tx = task_returned_tx.clone();

            // TODO(qti3e) The panic::set_hook sets the global panic hook, we need a per thread
            // panic hook, it can be created as a separate crate, this is how:
            // use std::sync::Once to setup a global panic handler that uses thread_local
            // to fetch the panic handler set for the current thread.
            // it should export per-thread set_hook and take_hook methods publicly.
            set_hook(Box::new(move |_| {
                block_on(async {
                    let trap_message = "Canister panicked";
                    unsafe {
                        ic0::trap(trap_message.as_ptr() as isize, trap_message.len() as isize)
                    };
                    task_panicked_tx.send(())
                        .await
                        .expect("ic-kit-runtime: Execution thread could not send task-completion signal to the main thread after panic.");
                });
            }));

            block_on(async {
                while let Some(task) = task_rx.recv().await {
                    task();
                    task_returned_tx.send(())
                        .await
                        .expect("ic-kit-runtime: Execution thread could not send task-completion signal to the main thread.")
                }
            });
        });

        Self {
            canister_id: Vec::from(Principal::from(canister_id).as_slice()),
            balance: 100_000_000_000_000,
            processing: None,
            execution_thread_handle: thread,
            task_tx,
            task_returned_rx,
            reply_tx,
            request_rx,
            symbol_table: HashMap::new(),
        }
    }

    /// Provide the canister with the definition of the given method.
    pub fn with_method<M: CanisterMethod + 'static>(mut self) -> Self {
        let method_name = String::from(M::EXPORT_NAME);
        let task_fn = Box::new(M::exported_method);

        if self.symbol_table.contains_key(&method_name) {
            panic!("The canister already has a '{}' method.", method_name);
        }

        self.symbol_table.insert(method_name, task_fn);
        self
    }

    /// Set the canister's cycle balance to this number.
    pub fn with_balance(mut self, balance: u128) -> Self {
        self.balance = balance;
        self
    }

    /// Send a request to the execution thread and waits until it's finished.
    pub async fn send(&mut self, request: Request) {
        self.task_tx
            .send(Box::new(|| {
                println!("Some function related to the request.")
            }))
            .await
            .unwrap_or_else(|_| {
                panic!("ic-kit-runtime: Could not send the task to the execution thread.")
            });

        loop {
            select! {
                Some(()) = self.task_returned_rx.recv() => {
                    // Okay the task returned successfully, we can give up.
                    return;
                },
                Some(req) = self.request_rx.recv() => {
                    let res = req.proxy(self);
                    self.reply_tx
                        .send(res)
                        .await
                        .expect("ic-kit-runtime: Could not send the system API call's response to the execution thread.");
                }
            }
        }
    }

    pub fn explicit_trap(&mut self, message: String) -> ! {
        panic!("Canister Trapped: {}", message)
    }
}

impl Ic0CallHandler for Canister {
    fn msg_arg_data_size(&mut self) -> isize {
        todo!()
    }

    fn msg_arg_data_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn msg_caller_size(&mut self) -> isize {
        todo!()
    }

    fn msg_caller_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn msg_reject_code(&mut self) -> i32 {
        todo!()
    }

    fn msg_reject_msg_size(&mut self) -> isize {
        todo!()
    }

    fn msg_reject_msg_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn msg_reply_data_append(&mut self, src: isize, size: isize) {
        todo!()
    }

    fn msg_reply(&mut self) {
        todo!()
    }

    fn msg_reject(&mut self, src: isize, size: isize) {
        todo!()
    }

    fn msg_cycles_available(&mut self) -> i64 {
        todo!()
    }

    fn msg_cycles_available128(&mut self, dst: isize) {
        todo!()
    }

    fn msg_cycles_refunded(&mut self) -> i64 {
        todo!()
    }

    fn msg_cycles_refunded128(&mut self, dst: isize) {
        todo!()
    }

    fn msg_cycles_accept(&mut self, max_amount: i64) -> i64 {
        todo!()
    }

    fn msg_cycles_accept128(&mut self, max_amount_high: i64, max_amount_low: i64, dst: isize) {
        todo!()
    }

    fn canister_self_size(&mut self) -> isize {
        todo!()
    }

    fn canister_self_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn canister_cycle_balance(&mut self) -> i64 {
        todo!()
    }

    fn canister_cycle_balance128(&mut self, dst: isize) {
        todo!()
    }

    fn canister_status(&mut self) -> i32 {
        todo!()
    }

    fn msg_method_name_size(&mut self) -> isize {
        todo!()
    }

    fn msg_method_name_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn accept_message(&mut self) {
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
    ) {
        todo!()
    }

    fn call_on_cleanup(&mut self, fun: isize, env: isize) {
        todo!()
    }

    fn call_data_append(&mut self, src: isize, size: isize) {
        todo!()
    }

    fn call_cycles_add(&mut self, amount: i64) {
        todo!()
    }

    fn call_cycles_add128(&mut self, amount_high: i64, amount_low: i64) {
        todo!()
    }

    fn call_perform(&mut self) -> i32 {
        todo!()
    }

    fn stable_size(&mut self) -> i32 {
        todo!()
    }

    fn stable_grow(&mut self, new_pages: i32) -> i32 {
        todo!()
    }

    fn stable_write(&mut self, offset: i32, src: isize, size: isize) {
        todo!()
    }

    fn stable_read(&mut self, dst: isize, offset: i32, size: isize) {
        todo!()
    }

    fn stable64_size(&mut self) -> i64 {
        todo!()
    }

    fn stable64_grow(&mut self, new_pages: i64) -> i64 {
        todo!()
    }

    fn stable64_write(&mut self, offset: i64, src: i64, size: i64) {
        todo!()
    }

    fn stable64_read(&mut self, dst: i64, offset: i64, size: i64) {
        todo!()
    }

    fn certified_data_set(&mut self, src: isize, size: isize) {
        todo!()
    }

    fn data_certificate_present(&mut self) -> i32 {
        todo!()
    }

    fn data_certificate_size(&mut self) -> isize {
        todo!()
    }

    fn data_certificate_copy(&mut self, dst: isize, offset: isize, size: isize) {
        todo!()
    }

    fn time(&mut self) -> i64 {
        todo!()
    }

    fn performance_counter(&mut self, counter_type: i32) -> i64 {
        todo!()
    }

    fn debug_print(&mut self, src: isize, size: isize) {
        todo!()
    }

    fn trap(&mut self, src: isize, size: isize) {
        todo!()
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Request::Init { .. } => f.write_str("init"),
            Request::Update { .. } => f.write_str("update"),
            Request::Query { .. } => f.write_str("query"),
            Request::Heartbeat => f.write_str("heartbeat"),
            Request::PostUpgrade => f.write_str("post_upgrade"),
            Request::PreUpgrade => f.write_str("init"),
            _ => f.write_str("X"),
        }
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
