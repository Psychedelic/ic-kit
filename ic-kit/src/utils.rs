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
