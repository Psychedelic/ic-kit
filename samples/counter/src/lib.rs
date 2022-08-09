use ic_kit::prelude::*;

#[derive(Default)]
pub struct Counter {
    number: u64,
}

impl Counter {
    /// Increment the counter by one.
    pub fn increment(&mut self) -> u64 {
        self.number += 1;
        self.number
    }

    /// Increment the counter by the provided value.
    pub fn increment_by(&mut self, n: u8) -> u64 {
        self.number += n as u64;
        self.number
    }
}

#[update]
pub fn increment() -> u64 {
    println!("Running increment.");
    with_mut(Counter::increment)
}

#[test]
fn x() {
    use ic_kit::rt;
    use ic_kit::rt::types::*;
    use rt::types::CanisterId;

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    rt.block_on(async {
        let canister_id = CanisterId::from_u64(12).into();
        let mut canister = rt::canister::Canister::new(canister_id).with_method::<increment>();
        let call = CanisterCall {
            request_id: RequestId::new(),
            callee: canister_id,
            method: "increment".to_string(),
            payment: 0,
            arg: Vec::from(ic::CANDID_EMPTY_ARG),
        };

        let (tx, rx) = rt::oneshot::channel();

        tokio::spawn(async move {
            println!("Making canister call.");
            canister.process_message(call.into(), Some(tx)).await;
        });

        println!("Waiting for response:");
        let response = rx.await.unwrap();
        println!("Response = {:?}", response);
    });
}
