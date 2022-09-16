use ic_kit::prelude::*;
use ic_kit_example_counter::CounterCanister;

#[cfg(target_family = "wasm")]
fn deploy<C: KitCanister>(_id: Principal) {
    unimplemented!()
}

#[cfg(not(target_family = "wasm"))]
fn deploy<C: KitCanister>(id: Principal) {
    use ic_kit::rt::Canister;

    let canister: Canister = CounterCanister::build(id);
    let canister = Box::leak(Box::new(canister));

    CallBuilder::new(Principal::management_canister(), "ic_kit_install")
        .with_arg(unsafe {
            let ptr = canister as *mut Canister as *mut _ as usize;
            ptr
        })
        .perform_one_way()
        .expect("ic-kit: could not install dynamic canister.");
}

#[update]
async fn deploy_counter() -> Principal {
    println!("Deploy counter!");

    let id = Principal::from_text("whq4n-xiaaa-aaaam-qaazq-cai").unwrap();
    deploy::<CounterCanister>(id);

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
