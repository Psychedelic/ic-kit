use ic_kit_sys::ic0;

/// The type used to represent the cycles amount, which is u128 when the `experimental-cycles128`
/// feature is on and a u64 otherwise.
#[cfg(feature = "experimental-cycles128")]
pub type Cycles = u128;

/// The type used to represent the cycles amount, which is u128 when the `experimental-cycles128`
/// feature is on and a u64 otherwise.
#[cfg(not(feature = "experimental-cycles128"))]
pub type Cycles = u64;

/// Return the number of available cycles that is sent by the caller.
pub fn msg_cycles_available() -> Cycles {
    #[cfg(not(feature = "experimental-cycles128"))]
    unsafe {
        ic0::msg_cycles_available() as u64
    }

    #[cfg(feature = "experimental-cycles128")]
    {
        let mut recv = 0u128;
        unsafe { ic0::msg_cycles_available128(&mut recv as *mut u128 as isize) }
        u128::from_le(recv)
    }
}

/// Accept the given amount of cycles, returns the actual amount of accepted cycles.
#[inline(always)]
pub fn msg_cycles_accept(max_amount: Cycles) -> Cycles {
    #[cfg(not(feature = "experimental-cycles128"))]
    unsafe {
        ic0::msg_cycles_accept(max_amount as i64) as u64
    }

    #[cfg(feature = "experimental-cycles128")]
    {
        if max_amount < (u64::MAX as u128) {
            return unsafe { ic0::msg_cycles_accept(max_amount as i64) as u128 };
        }

        let high = (max_amount >> 64) as u64 as i64;
        let low = (max_amount & (1 << 64)) as u64 as i64;
        let mut recv = 0u128;
        unsafe {
            ic0::msg_cycles_accept128(high, low, &mut recv as *mut u128 as isize);
        }
        u128::from_le(recv)
    }
}

/// Return the cycles that were sent back by the canister that was just called.
/// This method should only be called right after an inter-canister call.
#[inline(always)]
pub fn msg_cycles_refunded() -> Cycles {
    #[cfg(not(feature = "experimental-cycles128"))]
    unsafe {
        ic0::msg_cycles_refunded() as u64
    }

    #[cfg(feature = "experimental-cycles128")]
    {
        let mut recv = 0u128;
        unsafe { ic0::msg_cycles_refunded128(&mut recv as *mut u128 as isize) }
        u128::from_le(recv)
    }
}
