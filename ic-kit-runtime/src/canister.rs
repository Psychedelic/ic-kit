use crate::canister_id::CanisterId;
use crate::request::{Env, Message, RequestId};
use futures::executor::block_on;
use ic_kit_sys::ic0;
use ic_kit_sys::ic0::runtime;
use ic_kit_sys::ic0::Ic0CallHandler;
use ic_types::Principal;
use std::any::Any;
use std::collections::HashMap;
use std::panic::{catch_unwind, RefUnwindSafe};
use std::thread::JoinHandle;
use thread_local_panic_hook::set_hook;
use tokio::select;
use tokio::sync::mpsc::{self, Receiver, Sender};

///  A request ID for a request that is coming to this canister from the outside.
type IncomingRequestId = RequestId;
/// A request ID for a request that this canister has submitted.
type OutgoingRequestId = RequestId;

/// A canister that is being executed.
pub struct Canister {
    /// The id of the canister.
    canister_id: Vec<u8>,
    /// The canister balance.
    balance: u128,
    /// Maps the name of each of exported methods to the task function.
    symbol_table: HashMap<String, Box<dyn Fn() + Send + RefUnwindSafe>>,
    /// Map each incoming request id to the response buffer for it that is under construction.
    replies: HashMap<IncomingRequestId, Vec<u8>>,
    /// The canister execution environment.
    env: Env,
    /// The thread in which the canister is being executed at.
    execution_thread_handle: JoinHandle<()>,
    /// The communication channel to send tasks to the execution thread.
    task_tx: Sender<Box<dyn Fn() + Send + RefUnwindSafe>>,
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

impl Canister {
    pub fn new(canister_id: CanisterId) -> Self {
        let (request_tx, request_rx) = mpsc::channel(8);
        let (reply_tx, reply_rx) = mpsc::channel(8);
        let (task_tx, mut task_rx) = mpsc::channel::<Box<dyn Fn() + Send + RefUnwindSafe>>(8);
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
            set_hook(Box::new(move |m| {}));

            block_on(async {
                while let Some(task) = task_rx.recv().await {
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
                    task_completion_tx.send(c)
                        .await
                        .expect("ic-kit-runtime: Execution thread could not send task-completion signal to the main thread.");
                }
            });
        });

        Self {
            canister_id: Vec::from(Principal::from(canister_id).as_slice()),
            balance: 100_000_000_000_000,
            symbol_table: HashMap::new(),
            replies: HashMap::new(),
            env: Env::default(),
            execution_thread_handle,
            task_tx,
            task_completion_rx,
            reply_tx,
            request_rx,
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

    pub async fn process_message(&mut self, _message: Message) {
        // make sure we clean the task_returned receiver. since we may have sent more than one
        // completion signal from previous task.
        while let Ok(_) = self.task_completion_rx.try_recv() {}

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
                Some(_c) = self.task_completion_rx.recv() => {
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

    fn msg_arg_data_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn msg_caller_size(&mut self) -> isize {
        todo!()
    }

    fn msg_caller_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn msg_reject_code(&mut self) -> i32 {
        todo!()
    }

    fn msg_reject_msg_size(&mut self) -> isize {
        todo!()
    }

    fn msg_reject_msg_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn msg_reply_data_append(&mut self, _src: isize, _size: isize) {
        todo!()
    }

    fn msg_reply(&mut self) {
        todo!()
    }

    fn msg_reject(&mut self, _src: isize, _size: isize) {
        todo!()
    }

    fn msg_cycles_available(&mut self) -> i64 {
        todo!()
    }

    fn msg_cycles_available128(&mut self, _dst: isize) {
        todo!()
    }

    fn msg_cycles_refunded(&mut self) -> i64 {
        todo!()
    }

    fn msg_cycles_refunded128(&mut self, _dst: isize) {
        todo!()
    }

    fn msg_cycles_accept(&mut self, _max_amount: i64) -> i64 {
        todo!()
    }

    fn msg_cycles_accept128(&mut self, _max_amount_high: i64, _max_amount_low: i64, _dst: isize) {
        todo!()
    }

    fn canister_self_size(&mut self) -> isize {
        todo!()
    }

    fn canister_self_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn canister_cycle_balance(&mut self) -> i64 {
        todo!()
    }

    fn canister_cycle_balance128(&mut self, _dst: isize) {
        todo!()
    }

    fn canister_status(&mut self) -> i32 {
        todo!()
    }

    fn msg_method_name_size(&mut self) -> isize {
        todo!()
    }

    fn msg_method_name_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn accept_message(&mut self) {
        todo!()
    }

    fn call_new(
        &mut self,
        _callee_src: isize,
        _callee_size: isize,
        _name_src: isize,
        _name_size: isize,
        _reply_fun: isize,
        _reply_env: isize,
        _reject_fun: isize,
        _reject_env: isize,
    ) {
        todo!()
    }

    fn call_on_cleanup(&mut self, _fun: isize, _env: isize) {
        todo!()
    }

    fn call_data_append(&mut self, _src: isize, _size: isize) {
        todo!()
    }

    fn call_cycles_add(&mut self, _amount: i64) {
        todo!()
    }

    fn call_cycles_add128(&mut self, _amount_high: i64, _amount_low: i64) {
        todo!()
    }

    fn call_perform(&mut self) -> i32 {
        todo!()
    }

    fn stable_size(&mut self) -> i32 {
        todo!()
    }

    fn stable_grow(&mut self, _new_pages: i32) -> i32 {
        todo!()
    }

    fn stable_write(&mut self, _offset: i32, _src: isize, _size: isize) {
        todo!()
    }

    fn stable_read(&mut self, _dst: isize, _offset: i32, _size: isize) {
        todo!()
    }

    fn stable64_size(&mut self) -> i64 {
        todo!()
    }

    fn stable64_grow(&mut self, _new_pages: i64) -> i64 {
        todo!()
    }

    fn stable64_write(&mut self, _offset: i64, _src: i64, _size: i64) {
        todo!()
    }

    fn stable64_read(&mut self, _dst: i64, _offset: i64, _size: i64) {
        todo!()
    }

    fn certified_data_set(&mut self, _src: isize, _size: isize) {
        todo!()
    }

    fn data_certificate_present(&mut self) -> i32 {
        todo!()
    }

    fn data_certificate_size(&mut self) -> isize {
        todo!()
    }

    fn data_certificate_copy(&mut self, _dst: isize, _offset: isize, _size: isize) {
        todo!()
    }

    fn time(&mut self) -> i64 {
        todo!()
    }

    fn performance_counter(&mut self, _counter_type: i32) -> i64 {
        todo!()
    }

    fn debug_print(&mut self, _src: isize, _size: isize) {
        todo!()
    }

    fn trap(&mut self, _src: isize, _size: isize) {
        todo!()
    }
}

/// Copy the provided data
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

fn copy_from_canister(src: isize, size: isize) -> Vec<u8> {
    let src = src as usize;
    let size = size as usize;

    let slice = unsafe { std::slice::from_raw_parts(src as *const u8, size) };
    Vec::from(slice)
}

fn downcast_panic_payload(payload: &Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&'static str>()
        .cloned()
        .map(String::from)
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| String::from("Box<Any>"))
}
