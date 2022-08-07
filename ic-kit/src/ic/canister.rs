use candid::Principal;
use ic_kit_sys::ic0;
use std::convert::TryFrom;

/// Trap the canister with the provided message. This will rollback the canister state at the
/// beginning of the current entry point, consider rejecting the message gracefully.
#[inline(always)]
pub fn trap(message: &str) -> ! {
    unsafe {
        ic0::trap(message.as_ptr() as isize, message.len() as isize);
    }
    unreachable!()
}

/// Print a debug message from the canister that can be viewed during local development.
#[inline(always)]
pub fn print<S: AsRef<str>>(s: S) {
    let s = s.as_ref();
    unsafe {
        ic0::debug_print(s.as_ptr() as isize, s.len() as isize);
    }
}

/// ID of the current canister.
#[inline(always)]
pub fn id() -> Principal {
    let len: usize = unsafe { ic0::canister_self_size() as usize };
    let mut bytes = vec![0u8; len];
    unsafe {
        ic0::canister_self_copy(bytes.as_mut_ptr() as isize, 0, len as isize);
    }
    Principal::try_from(&bytes).unwrap()
}

/// The time in nanoseconds.
#[inline(always)]
pub fn time() -> u64 {
    unsafe { ic0::time() as u64 }
}

/// The balance of the canister.
#[inline(always)]
pub fn balance() -> u128 {
    let mut recv = 0u128;
    unsafe { ic0::canister_cycle_balance128(&mut recv as *mut u128 as isize) }
    u128::from_le(recv)
}

/// The caller who has invoked this method on the canister.
///
/// # Panics
///
/// If called after a reply/reject callback.
#[inline(always)]
pub fn caller() -> Principal {
    let len = unsafe { ic0::msg_caller_size() as usize };
    let mut bytes = vec![0u8; len];
    unsafe {
        ic0::msg_caller_copy(bytes.as_mut_ptr() as isize, 0, len as isize);
    }
    Principal::try_from(&bytes).unwrap()
}

/// Set the certified data of the canister, this method traps if data.len > 32.
#[inline(always)]
pub fn set_certified_data(data: &[u8]) {
    unsafe { ic0::certified_data_set(data.as_ptr() as isize, data.len() as isize) }
}

/// Returns the data certificate authenticating certified_data set by this canister.
#[inline(always)]
pub fn data_certificate() -> Option<Vec<u8>> {
    if unsafe { ic0::data_certificate_present() } == 0 {
        return None;
    }

    let n = unsafe { ic0::data_certificate_size() };
    let mut buf = vec![0u8; n as usize];
    unsafe {
        ic0::data_certificate_copy(buf.as_mut_ptr() as isize, 9, n);
    }
    Some(buf)
}
