use super::{KeyBound, RbTree};
use crate::{AsHashTree, HashTree};
use std::convert::AsRef;

#[test]
fn test_witness() {
    let mut t = RbTree::<Vec<u8>, Vec<u8>>::new();

    for i in 0u64..10 {
        let key = (1 + 2 * i).to_be_bytes();
        let val = (1 + 2 * i).to_le_bytes();
        t.insert(key.into(), val.into());
        assert_eq!(t.get(&key[..]).map(|v| &v[..]), Some(&val[..]));
    }

    for i in 0u64..10 {
        let key = (1 + 2 * i).to_be_bytes();
        let key = key.as_ref();

        let ht = t.witness(key);
        assert_eq!(
            ht.reconstruct(),
            t.root_hash(),
            "key: {}, witness {:?}",
            hex::encode(key),
            ht
        );

        let ht = t.keys_with_prefix(key);
        assert_eq!(
            ht.reconstruct(),
            t.root_hash(),
            "key: {}, lower bound: {:?}, upper_bound: {:?}, witness {:?}",
            hex::encode(key),
            t.lower_bound(key).map(hex::encode),
            t.right_prefix_neighbor(key).map(hex::encode),
            ht
        );
    }

    for i in 0u64..10 {
        for j in i..10 {
            let start = (2 * i).to_be_bytes();
            let end = (2 * j).to_be_bytes();
            let ht = t.key_range(&start[..], &end[..]);
            assert_eq!(
                ht.reconstruct(),
                t.root_hash(),
                "key range: [{}, {}], witness {:?}",
                hex::encode(&start[..]),
                hex::encode(&end[..]),
                ht
            );

            let ht = t.value_range(&start[..], &end[..]);
            assert_eq!(
                ht.reconstruct(),
                t.root_hash(),
                "key range: [{}, {}], witness {:?}",
                hex::encode(&start[..]),
                hex::encode(&end[..]),
                ht
            );
        }
    }

    for i in 0u64..11 {
        let key = (2 * i).to_be_bytes();
        let ht = t.witness(&key[..]);
        assert_eq!(
            ht.reconstruct(),
            t.root_hash(),
            "key: {}, witness {:?}",
            hex::encode(&key[..]),
            ht
        );
    }

    for i in 0u64..10 {
        let key = (1 + 2 * i).to_be_bytes();
        let val = (1 + 2 * i).to_le_bytes();

        assert_eq!(t.get(&key[..]).map(|v| &v[..]), Some(&val[..]));

        t.delete(&key[..]);
        for j in 0u64..10 {
            let witness_key = (1 + 2 * j).to_be_bytes();
            let ht = t.witness(&witness_key[..]);
            assert_eq!(
                ht.reconstruct(),
                t.root_hash(),
                "key: {}, witness {:?}",
                hex::encode(&key[..]),
                ht
            );
        }
        assert_eq!(t.get(&key[..]), None);
    }
}

#[test]
fn test_key_bounds() {
    let mut t = RbTree::<Vec<u8>, Vec<u8>>::new();
    t.insert(vec![1], vec![10]);
    t.insert(vec![3], vec![30]);

    assert_eq!(t.lower_bound(&[0u8][..]), None);
    assert_eq!(t.lower_bound(&[1u8][..]), Some(KeyBound::Exact(&vec![1u8])));
    assert_eq!(
        t.lower_bound(&[2u8][..]),
        Some(KeyBound::Neighbor(&vec![1u8]))
    );
    assert_eq!(t.lower_bound(&[3u8][..]), Some(KeyBound::Exact(&vec![3u8])));
    assert_eq!(
        t.lower_bound(&[4u8][..]),
        Some(KeyBound::Neighbor(&vec![3u8]))
    );

    assert_eq!(
        t.upper_bound(&[0u8][..]),
        Some(KeyBound::Neighbor(&vec![1u8]))
    );
    assert_eq!(t.upper_bound(&[1u8][..]), Some(KeyBound::Exact(&vec![1u8])));
    assert_eq!(
        t.upper_bound(&[2u8][..]),
        Some(KeyBound::Neighbor(&vec![3u8]))
    );
    assert_eq!(t.upper_bound(&[3u8][..]), Some(KeyBound::Exact(&vec![3u8])));
    assert_eq!(t.upper_bound(&[4u8][..]), None);
}

#[test]
fn test_prefix_neighbor() {
    let mut t = RbTree::<String, Vec<u8>>::new();
    t.insert("a/b".into(), vec![0]);
    t.insert("a/b".into(), vec![0]);
    t.insert("a/b/c".into(), vec![1]);
    t.insert("a/b/d".into(), vec![2]);
    t.insert("a/c/d".into(), vec![3]);

    assert_eq!(
        t.right_prefix_neighbor("a/b/c"),
        Some(KeyBound::Neighbor(&"a/b/d".into()))
    );
    assert_eq!(
        t.right_prefix_neighbor("a/b"),
        Some(KeyBound::Neighbor(&"a/c/d".into()))
    );
    assert_eq!(t.right_prefix_neighbor("a/c/d"), None);
    assert_eq!(t.right_prefix_neighbor("a"), None);
}

#[test]
fn simple_delete_test() {
    let mut t = RbTree::<String, String>::new();
    t.insert("x".into(), "a".into());
    t.insert("y".into(), "b".into());
    t.insert("z".into(), "c".into());

    t.delete("x");
    assert_eq!(t.get("x"), None);
    assert_eq!(t.get("y"), Some(&"b".into()));
    assert_eq!(t.get("z"), Some(&"c".into()));

    t.delete("y");
    assert_eq!(t.get("y"), None);
    assert_eq!(t.get("z"), Some(&"c".into()));

    t.delete("z");
    assert_eq!(t.get("z"), None);
}

#[test]
fn simple_delete_test_2() {
    let mut t = RbTree::<String, String>::new();
    t.insert("x".into(), "y".into());
    t.insert("z".into(), "w".into());

    t.delete("z");
    assert_eq!(t.get("z"), None);
    assert_eq!(t.get("x"), Some(&"y".into()));
}

#[test]
fn map_model_test() {
    use std::collections::HashMap;

    let mut hm: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    let mut rb = RbTree::<Vec<u8>, Vec<u8>>::new();

    for i in 0..100u64 {
        hm.insert(i.to_be_bytes().to_vec(), i.to_be_bytes().to_vec());
        rb.insert(i.to_be_bytes().to_vec(), i.to_be_bytes().to_vec());

        for k in hm.keys() {
            assert_eq!(hm.get(k), rb.get(k));
        }
    }
    let keys: Vec<_> = hm.keys().cloned().collect();

    for k in keys {
        hm.remove(&k);

        assert!(rb.get(&k).is_some());
        rb.delete(&k);
        assert!(rb.get(&k).is_none());

        for k in hm.keys() {
            assert_eq!(hm.get(k), rb.get(k));
        }
    }
    assert_eq!(super::debug_alloc::count_allocated_pointers(), 0);
}

#[test]
fn test_nested_witness() {
    let mut rb: RbTree<String, RbTree<String, String>> = RbTree::new();
    let mut nested = RbTree::new();
    nested.insert("bottom".into(), "data".into());
    rb.insert("top".into(), nested);

    let ht = rb.nested_witness("top", |v| v.witness("bottom"));

    assert_eq!(ht.reconstruct(), rb.root_hash());
    match ht {
        HashTree::Labeled(lt, tt) => {
            assert_eq!(lt.as_ref(), b"top");
            match &(*tt) {
                HashTree::Labeled(lb, _) => {
                    assert_eq!(lb.as_ref(), b"bottom");
                }
                other => panic!("unexpected nested tree: {:?}", other),
            }
        }
        other => panic!("expected a labeled tree, got {:?}", other),
    }

    rb.modify("top", |m| {
        m.delete("bottom");
    });
    let ht = rb.nested_witness("top", |v| v.witness("bottom"));
    assert_eq!(ht.reconstruct(), rb.root_hash());
}

#[test]
fn test_witness_key_range() {
    let mut t = RbTree::<String, String>::new();
    t.insert("b".into(), "x".into());
    t.insert("d".into(), "y".into());
    t.insert("f".into(), "z".into());

    assert_eq!(t.key_range("a", "a").get_labels(), vec![b"b"]);
    assert_eq!(t.key_range("a", "b").get_labels(), vec![b"b"]);
    assert_eq!(t.key_range("a", "c").get_labels(), vec![b"b", b"d"]);
    assert_eq!(t.key_range("a", "f").get_labels(), vec![b"b", b"d", b"f"]);
    assert_eq!(t.key_range("a", "z").get_labels(), vec![b"b", b"d", b"f"]);

    assert_eq!(t.key_range("b", "b").get_labels(), vec![b"b"]);
    assert_eq!(t.key_range("b", "c").get_labels(), vec![b"b", b"d"]);
    assert_eq!(t.key_range("b", "f").get_labels(), vec![b"b", b"d", b"f"]);
    assert_eq!(t.key_range("b", "z").get_labels(), vec![b"b", b"d", b"f"]);

    assert_eq!(t.key_range("d", "e").get_labels(), vec![b"d", b"f"]);
    assert_eq!(t.key_range("d", "f").get_labels(), vec![b"d", b"f"]);
    assert_eq!(t.key_range("d", "z").get_labels(), vec![b"d", b"f"]);
    assert_eq!(t.key_range("y", "z").get_labels(), vec![b"f"]);

    assert!(t.key_range("a", "z").get_leaf_values().is_empty());
}

#[test]
fn test_witness_value_range() {
    let mut t = RbTree::<String, String>::new();
    t.insert("b".into(), "x".into());
    t.insert("d".into(), "y".into());
    t.insert("f".into(), "z".into());

    assert_eq!(t.key_range("a", "a").get_labels(), vec![b"b"]);
    assert!(t.value_range("a", "a").get_leaf_values().is_empty());

    assert_eq!(t.value_range("a", "b").get_labels(), vec![b"b"]);
    assert_eq!(t.value_range("a", "b").get_leaf_values(), vec![b"x"]);

    assert_eq!(t.value_range("f", "z").get_labels(), vec![b"f"]);
    assert_eq!(t.value_range("f", "z").get_leaf_values(), vec![b"z"]);

    assert_eq!(t.value_range("g", "z").get_labels(), vec![b"f"]);
    assert!(t.value_range("g", "z").get_leaf_values().is_empty());

    assert_eq!(t.value_range("a", "z").get_labels(), vec![b"b", b"d", b"f"]);
    assert_eq!(
        t.value_range("a", "z").get_leaf_values(),
        vec![b"x", b"y", b"z"]
    );
}
