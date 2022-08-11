#![feature(future_join)]
use ic_kit::prelude::*;
use std::future::join;

#[update]
async fn fib(n: u64) -> u64 {
    println!("fib({})", n);
    if n > 20 {
        ic::trap("Let's not kill IC.");
    }

    if n <= 1 {
        return n;
    }

    let a = CallBuilder::new(id(), "fib")
        .with_arg(n - 1)
        .perform_one::<u64>()
        .await
        .unwrap();

    let b = CallBuilder::new(id(), "fib")
        .with_arg(n - 2)
        .perform_one::<u64>()
        .await
        .unwrap();

    a + b
}

#[update]
async fn fib_join(n: u64) -> u64 {
    if n > 20 {
        ic::trap("Let's not kill IC.");
    }

    if n <= 1 {
        return n;
    }

    let a_call = CallBuilder::new(id(), "fib_join").with_arg(n - 1);
    let b_call = CallBuilder::new(id(), "fib_join").with_arg(n - 2);

    let (a, b) = join!(a_call.perform_one::<u64>(), b_call.perform_one::<u64>()).await;

    let a = a.unwrap();
    let b = b.unwrap();

    a + b
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct FibCanister;

#[cfg(test)]
mod test {
    use super::*;

    #[kit_test]
    async fn fib_6(replica: Replica) {
        let canister = replica.add_canister(FibCanister::anonymous());

        let fib_6 = canister
            .new_call("fib")
            .with_caller(*users::ALICE)
            .with_arg(6u64)
            .perform()
            .await
            .decode_one::<u64>()
            .unwrap();

        assert_eq!(fib_6, 8);
    }

    #[kit_test]
    async fn fib_spawn_6(replica: Replica) {
        let canister = replica.add_canister(FibCanister::anonymous());

        let fib_6 = canister
            .new_call("fib_join")
            .with_caller(*users::ALICE)
            .with_arg(6u64)
            .perform()
            .await
            .decode_one::<u64>()
            .unwrap();

        assert_eq!(fib_6, 8);
    }
}
