use ic_kit::prelude::*;
use ic_kit_example_counter::CounterCanister;

use ic_kit::rt::Canister;

#[update]
async fn deploy_counter() -> Principal {
    println!("Deploy counter!");

    let id = Principal::from_text("whq4n-xiaaa-aaaam-qaazq-cai").unwrap();
    let canister: Canister = CounterCanister::build(id);
    let canister = Box::leak(Box::new(canister));

    let r = CallBuilder::new(Principal::management_canister(), "ic_kit_install")
        .with_arg(unsafe {
            let ptr = canister as *mut Canister as *mut _ as usize;
            ptr
        })
        .perform_one_way();

    println!("{:#?}", r);

    id
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct FactoryCounterCanister;

#[cfg(test)]
mod tests {
    use super::*;

    #[kit_test]
    async fn x(replica: Replica) {
        let factory = replica.add_canister(FactoryCounterCanister::anonymous());
        let new_canister_id = factory
            .new_call("deploy_counter")
            .perform()
            .await
            .decode_one::<Principal>()
            .unwrap();

        let counter = replica.get_canister(new_canister_id);
        let r = counter
            .new_call("increment")
            .perform()
            .await
            .decode_one::<u64>()
            .unwrap();

        assert_eq!(r, 1);

        assert_eq!(
            counter
                .new_call("get_counter")
                .perform()
                .await
                .decode_one::<u64>()
                .unwrap(),
            1
        );
    }
}
