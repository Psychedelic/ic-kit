// This file is copied from ic-certified-map which was released under Apache V2.
// Some modifications are made to improve the code quality.
use serde::{ser::SerializeSeq, Serialize, Serializer};
use serde_bytes::Bytes;
use sha2::{Digest, Sha256};
use std::borrow::Cow;

/// SHA-256 hash bytes.
pub type Hash = [u8; 32];

#[derive(Debug, Eq, PartialEq)]
pub struct ForkInner<'a>(pub HashTree<'a>, pub HashTree<'a>);

impl<'a> ForkInner<'a> {
    pub fn left(&self) -> &HashTree<'a> {
        &self.0
    }

    pub fn right(&self) -> &HashTree<'a> {
        &self.1
    }
}

/// HashTree as defined in the interfaces spec.
/// https://sdk.dfinity.org/docs/interface-spec/index.html#_certificate
#[derive(Debug, Eq, PartialEq)]
pub enum HashTree<'a> {
    Empty,
    Fork(Box<ForkInner<'a>>),
    Labeled(Cow<'a, [u8]>, Box<HashTree<'a>>),
    Leaf(Cow<'a, [u8]>),
    Pruned(Hash),
}

pub fn fork<'a>(l: HashTree<'a>, r: HashTree<'a>) -> HashTree<'a> {
    HashTree::Fork(Box::new(ForkInner(l, r)))
}

pub fn labeled<'a>(l: &'a [u8], t: HashTree<'a>) -> HashTree<'a> {
    HashTree::Labeled(Cow::Borrowed(l), Box::new(t))
}

pub fn fork_hash(l: &Hash, r: &Hash) -> Hash {
    let mut h = domain_sep("ic-hashtree-fork");
    h.update(&l[..]);
    h.update(&r[..]);
    h.finalize().into()
}

pub fn leaf_hash(data: &[u8]) -> Hash {
    let mut h = domain_sep("ic-hashtree-leaf");
    h.update(data);
    h.finalize().into()
}

pub fn labeled_hash(label: &[u8], content_hash: &Hash) -> Hash {
    let mut h = domain_sep("ic-hashtree-labeled");
    h.update(label);
    h.update(&content_hash[..]);
    h.finalize().into()
}

impl<'a> HashTree<'a> {
    pub fn reconstruct(&self) -> Hash {
        match self {
            Self::Empty => domain_sep("ic-hashtree-empty").finalize().into(),
            Self::Fork(f) => fork_hash(&f.0.reconstruct(), &f.1.reconstruct()),
            Self::Labeled(l, t) => {
                let thash = t.reconstruct();
                labeled_hash(l, &thash)
            }
            Self::Leaf(data) => leaf_hash(data),
            Self::Pruned(h) => *h,
        }
    }

    /// Collect and return all of the labels in this HashTree.
    ///
    /// This method is intended for testing purposes.
    pub fn get_labels<'b: 'a>(&'b self) -> Vec<&'b [u8]> {
        fn go<'a>(keys: &mut Vec<&'a [u8]>, tree: &'a HashTree<'a>) {
            match tree {
                HashTree::Labeled(key, value) => {
                    keys.push(key);
                    go(keys, value);
                }
                HashTree::Fork(lr) => {
                    go(keys, lr.left());
                    go(keys, lr.right());
                }
                _ => (),
            }
        }

        let mut keys = Vec::new();
        go(&mut keys, self);
        keys
    }

    /// Collect and return all of the values in this HashTree.
    ///
    /// This method is intended for testing purposes.
    pub fn get_leaf_values<'b: 'a>(&'b self) -> Vec<&'b [u8]> {
        fn go<'a>(values: &mut Vec<&'a [u8]>, tree: &'a HashTree<'a>) {
            match tree {
                HashTree::Leaf(value) => {
                    values.push(value);
                }
                HashTree::Fork(lr) => {
                    go(values, lr.left());
                    go(values, lr.right());
                }
                HashTree::Labeled(_, t) => {
                    go(values, &*t);
                }
                _ => (),
            }
        }

        let mut values = Vec::new();
        go(&mut values, self);
        values
    }
}

impl Serialize for HashTree<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        match self {
            HashTree::Empty => {
                let mut seq = serializer.serialize_seq(Some(1))?;
                seq.serialize_element(&0u8)?;
                seq.end()
            }
            HashTree::Fork(p) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&1u8)?;
                seq.serialize_element(&p.0)?;
                seq.serialize_element(&p.1)?;
                seq.end()
            }
            HashTree::Labeled(label, tree) => {
                let mut seq = serializer.serialize_seq(Some(3))?;
                seq.serialize_element(&2u8)?;
                seq.serialize_element(Bytes::new(label))?;
                seq.serialize_element(&tree)?;
                seq.end()
            }
            HashTree::Leaf(leaf_bytes) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&3u8)?;
                seq.serialize_element(Bytes::new(leaf_bytes))?;
                seq.end()
            }
            HashTree::Pruned(digest) => {
                let mut seq = serializer.serialize_seq(Some(2))?;
                seq.serialize_element(&4u8)?;
                seq.serialize_element(Bytes::new(&digest[..]))?;
                seq.end()
            }
        }
    }
}

fn domain_sep(s: &str) -> sha2::Sha256 {
    let buf: [u8; 1] = [s.len() as u8];
    let mut h = Sha256::new();
    h.update(&buf[..]);
    h.update(s.as_bytes());
    h
}

#[cfg(test)]
mod tests {
    use super::{
        fork, labeled,
        HashTree::{Empty, Leaf},
    };
    use std::borrow::Cow;

    //─┬─┬╴"a" ─┬─┬╴"x" ─╴"hello"
    // │ │      │ └╴Empty
    // │ │      └╴  "y" ─╴"world"
    // │ └╴"b" ──╴"good"
    // └─┬╴"c" ──╴Empty
    //   └╴"d" ──╴"morning"
    #[test]
    fn test_public_spec_example() {
        let t = fork(
            fork(
                labeled(
                    b"a",
                    fork(
                        fork(labeled(b"x", Leaf(Cow::Borrowed(b"hello"))), Empty),
                        labeled(b"y", Leaf(Cow::Borrowed(b"world"))),
                    ),
                ),
                labeled(b"b", Leaf(Cow::Borrowed(b"good"))),
            ),
            fork(
                labeled(b"c", Empty),
                labeled(b"d", Leaf(Cow::Borrowed(b"morning"))),
            ),
        );

        assert_eq!(
            hex::encode(&t.reconstruct()[..]),
            "eb5c5b2195e62d996b84c9bcc8259d19a83786a2f59e0878cec84c811f669aa0".to_string()
        );

        assert_eq!(
            hex::encode(serde_cbor::to_vec(&t).unwrap()),
            "8301830183024161830183018302417882034568656c6c6f810083024179820345776f726c6483024162820344676f6f648301830241638100830241648203476d6f726e696e67".to_string());
    }
}
