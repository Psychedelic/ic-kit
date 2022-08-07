use ic_kit_sys::ic0;

/// Return the raw argument data to this entry point.
pub fn arg_data_raw() -> Vec<u8> {
    unsafe {
        let len: usize = ic0::msg_arg_data_size() as usize;
        if len == 0 {
            return Vec::new();
        }

        let mut bytes = Vec::with_capacity(len);
        ic0::msg_arg_data_copy(bytes.as_mut_ptr() as isize, 0, len as isize);
        bytes.set_len(len);
        bytes
    }
}

/// Reply to the current call using the provided buffer.
pub fn reply(buf: &[u8]) {
    unsafe {
        if !buf.is_empty() {
            ic0::msg_reply_data_append(buf.as_ptr() as isize, buf.len() as isize)
        }
        ic0::msg_reply()
    }
}

/// Reject the current call.
pub fn reject(message: &str) {
    unsafe { ic0::msg_reject(message.as_ptr() as isize, message.len() as isize) }
}

/// Accept the incoming message.
pub fn accept() {
    unsafe {
        ic0::accept_message();
    }
}

/// Return the name of the current canister method.
pub fn method_name() -> String {
    let len = unsafe { ic0::msg_method_name_size() as usize };
    let mut bytes = vec![0u8; len];
    unsafe {
        ic0::msg_method_name_copy(bytes.as_mut_ptr() as isize, 0, len as isize);
    }
    String::from_utf8_lossy(&bytes).to_string()
}

/// Get the value of specified performance counter.
///
/// Supported counter type:
///
/// 0 : Instruction counter.  
///     The number of WebAssembly instructions the system has determined that the canister has executed.
pub fn performance_counter(counter_type: u32) -> u64 {
    unsafe { ic0::performance_counter(counter_type as i32) as u64 }
}
