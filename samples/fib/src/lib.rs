use ic_kit::prelude::*;

#[update]
async fn fib(n: u64) -> u64 {
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::rt::types::{CanisterCall, RequestId};
    use ic_kit::candid::{decode_one, encode_one};

    #[kit_test]
    async fn x(replica: Replica) {
        let canister_id = CanisterId::from_u64(124).into();
        replica.add_canister(Canister::new(canister_id).with_method::<fib>());

        let call = CanisterCall {
            sender: Principal::from(CanisterId::from_u64(12)),
            request_id: RequestId::new(),
            callee: canister_id,
            method: "fib".to_string(),
            payment: 0,
            arg: encode_one(6u64).unwrap(),
        };

        let response = replica.perform(call).await;
        assert_eq!(
            decode_one::<u64>(&response.to_result().unwrap()).unwrap(),
            8
        );
    }
}
