use ic_kit::prelude::*;
use std::collections::HashSet;

#[derive(Default)]
struct MultiCounter {
    canister_ids: HashSet<Principal>,
}

#[update]
fn increment(counters: &MultiCounter) {
    println!("MultiCounter Increment!");

    for &canister_id in counters.canister_ids.iter() {
        println!("Increment on {}", canister_id);

        CallBuilder::new(canister_id, "increment")
            .perform_one_way()
            .expect("Expected the one way call to succeed.");
    }
}

#[update]
fn add_counter(counters: &mut MultiCounter, canister_id: Principal) {
    println!("Add counter: {}", canister_id);
    counters.canister_ids.insert(canister_id);
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct MultiCounterCanister;

#[cfg(test)]
mod tests {
    use super::*;
    use ic_kit_example_counter::CounterCanister;

    #[kit_test]
    async fn test_multi_canister(replica: Replica) {
        let counter1_id = Principal::from_text("whq4n-xiaaa-aaaam-qaazq-cai").unwrap();
        let counter2_id = Principal::from_text("lj532-6iaaa-aaaah-qcc7a-cai").unwrap();

        let canister = replica.add_canister(MultiCounterCanister::anonymous());
        let counter1 = replica.add_canister(CounterCanister::build(counter1_id));
        let counter2 = replica.add_canister(CounterCanister::build(counter2_id));

        // Test individual counters.

        let r = counter1
            .new_call("increment")
            .perform()
            .await
            .decode_one::<u64>()
            .unwrap();

        assert_eq!(r, 1);

        let r = counter2
            .new_call("increment")
            .perform()
            .await
            .decode_one::<u64>()
            .unwrap();

        assert_eq!(r, 1);

        // Add the counters to the multi-counter.

        canister
            .new_call("add_counter")
            .with_arg(&counter1_id)
            .perform()
            .await;

        canister
            .new_call("add_counter")
            .with_arg(&counter2_id)
            .perform()
            .await;

        // Do a proxy increment call.
        let x = canister.new_call("increment").perform().await;

        // TODO(qti3e) replica.idle.await

        println!("{:#?}", x);
    }
}
