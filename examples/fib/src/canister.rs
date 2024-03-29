use ic_kit::prelude::*;

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
}
