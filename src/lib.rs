#[cfg(target_family = "wasm")]
mod ic;
mod inject;
mod interface;
mod mock;

#[cfg(target_family = "wasm")]
pub use ic::*;
pub use interface::*;
pub use mock::*;

pub use ic_cdk::export::candid;
pub use ic_cdk::export::Principal;

pub mod macros {
    /// Re-export async_std test to be used for async tests when not targeting WASM.
    #[cfg(not(target_family = "wasm"))]
    pub use async_std::test;

    /// Re-export ic_cdk_macros.
    pub use ic_cdk_macros::*;
}

/// The type definition of common canisters on the Internet Computer.
pub mod interfaces;

/// Return the IC context depending on the build target.
#[inline(always)]
pub fn get_context() -> &'static mut impl Context {
    #[cfg(not(target_family = "wasm"))]
    return inject::get_context();
    #[cfg(target_family = "wasm")]
    return IcContext::context();
}

#[cfg(test)]
mod demo {
    use super::*;
    use crate::interfaces::management::WithCanisterId;
    use crate::interfaces::{management, Method};

    async fn deposit_to_canister(cycles: u64) {
        let ic = get_context();
        let balance = ic.get_mut::<u64>();

        if cycles > *balance {
            panic!("Not enough balance");
        }

        *balance -= cycles;

        match management::DepositCycles::perform_with_payment(
            ic,
            Principal::management_canister(),
            (WithCanisterId {
                canister_id: Principal::anonymous(),
            },),
            cycles,
        )
        .await
        {
            Ok(()) => {}
            Err((r, c)) => {
                todo!()
            }
        };

        *balance += ic.msg_cycles_refunded();
    }

    #[macros::test]
    async fn test_refund() {
        let ctx = MockContext::new()
            .with_balance(2000)
            .with_data(1000u64)
            .with_canister(
                Principal::management_canister(),
                MockCanister::new().with_method(
                    "deposit_cycles",
                    |ctx, args: (WithCanisterId,)| {
                        ctx.msg_cycles_accept(100);
                        Ok(())
                    },
                ),
            )
            .inject();

        deposit_to_canister(500).await;

        assert_eq!(ctx.balance(), 1900);
        assert_eq!(ctx.get::<u64>(), &900);
    }
}
