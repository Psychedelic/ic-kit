use ic_kit::prelude::*;
use std::collections::HashMap;

#[derive(Default)]
struct Registry {
    names: HashMap<Principal, String>,
}

#[derive(Default)]
struct Stats {
    called_register: u64,
}

#[update]
fn register(registry: &mut Registry, stats: &mut Stats, name: String) {
    stats.called_register += 1;
    registry.names.insert(caller(), name);
}

#[query]
fn get_name(registry: &Registry, user: Principal) -> Option<&String> {
    registry.names.get(&user)
}

#[derive(KitCanister)]
#[candid_path("candid.did")]
pub struct NamingSystemCanister;

#[cfg(test)]
mod tests {
    use super::*;

    #[kit_test]
    async fn test(replica: Replica) {
        let ns = replica.add_canister(NamingSystemCanister::anonymous());

        ns.new_call("register")
            .with_caller(*users::ALICE)
            .with_arg("Alice")
            .perform()
            .await
            .assert_ok();

        ns.new_call("register")
            .with_caller(*users::BOB)
            .with_arg("Bob")
            .perform()
            .await
            .assert_ok();

        let alice_name = ns
            .new_call("get_name")
            .with_arg(*users::ALICE)
            .perform()
            .await
            .decode_one::<Option<String>>()
            .unwrap();

        assert_eq!(alice_name, Some("Alice".to_string()));

        let bob_name = ns
            .new_call("get_name")
            .with_arg(*users::BOB)
            .perform()
            .await
            .decode_one::<Option<String>>()
            .unwrap();

        assert_eq!(bob_name, Some("Bob".to_string()));
    }
}
