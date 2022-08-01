use actix::prelude::*;
use ic_kit_sys::ic0::Ic0CallHandler;
use std::fmt::{Display, Formatter};

#[derive(Default)]
pub struct Runtime {
    /// The request we're currently processing.
    processing: Option<Request>,
    /// Inter-canister calls to perform if the current methods does not trap.
    perform_calls: Vec<Call>,
    /// The current call that is being constructed. When `call_perform` gets called we will push
    /// this value to self.calls.
    call_factory: Option<IncompleteCall>,
}

pub enum Request {
    Init {
        arg_data: Vec<u8>,
    },
    Update {
        arg_data: Vec<u8>,
    },
    Query {
        arg_data: Vec<u8>,
    },
    CanisterInspect {
        arg_data: Vec<u8>,
        method_name: String,
    },
    ReplyCallback {
        arg_data: Vec<u8>,
    },
    RejectCallback {},
    CleanupCallback {},
    Heartbeat,
    PostUpgrade,
    PreUpgrade,
}

pub struct Call {}

pub struct IncompleteCall {}

impl Runtime {
    pub fn explicit_trap(&mut self, message: String) -> ! {
        panic!("Trapped!")
    }
}

impl Ic0CallHandler for Runtime {
    fn msg_arg_data_size(&mut self) -> isize {
        let req = self
            .processing
            .as_ref()
            .expect("Unexpected: No request is being processed.");

        match req {
            Request::Init { arg_data, .. } => arg_data.len() as isize,
            Request::Update { arg_data, .. } => arg_data.len() as isize,
            Request::Query { arg_data, .. } => arg_data.len() as isize,
            Request::CanisterInspect { arg_data, .. } => arg_data.len() as isize,
            Request::ReplyCallback { arg_data, .. } => arg_data.len() as isize,
            _ => self.explicit_trap(format!("ic0::msg_arg_data_size called from '{}'")),
        }
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
            Request::CanisterInspect { .. } => f.write_str("inspect"),
            Request::ReplyCallback { .. } => f.write_str("reply_callback"),
            Request::RejectCallback { .. } => f.write_str("reject_callback"),
            Request::CleanupCallback { .. } => f.write_str("cleanup_callback"),
            Request::Heartbeat => f.write_str("heartbeat"),
            Request::PostUpgrade => f.write_str("post_upgrade"),
            Request::PreUpgrade => f.write_str("init"),
        }
    }
}

#[test]
fn x() {
    let mut runtime = Runtime::default();
    unsafe {
        let mut bytes = Vec::<u8>::with_capacity(100);
        let ptr64 = bytes.as_mut_ptr() as i64;
        let ptr32 = bytes.as_mut_ptr() as i32;
        let ptr_isize = bytes.as_mut_ptr() as isize;
        println!("ptr64 = {}", ptr64);
        println!("ptr32 = {}", ptr32);
        println!("ptris = {}", ptr_isize);
    }
}
