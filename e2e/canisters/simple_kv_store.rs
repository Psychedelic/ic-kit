use ic_kit::macros::{post_upgrade, pre_upgrade, query, update};
use serde_bytes::ByteBuf;
use std::collections::BTreeMap;

type Store = BTreeMap<String, ByteBuf>;

#[update]
fn insert(store: &mut Store, key: String, value: ByteBuf) {
    store.insert(key, value);
}

#[query]
fn lookup(store: &Store, key: String) -> Option<&ByteBuf> {
    store.get(&key)
}

#[pre_upgrade]
fn pre_upgrade(store: &Store) {
    ic_kit::stable::stable_store((store,)).unwrap();
}

#[post_upgrade]
fn post_upgrade() {
    let (persisted_store,): (Store,) = ic_kit::stable::stable_restore().unwrap();
    ic_kit::ic::swap(persisted_store);
}

fn main() {}
