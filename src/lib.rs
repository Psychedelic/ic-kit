use std::collections::BTreeMap;
use ic_cdk::export::Principal;

pub struct TestContext {
    handlers: BTreeMap<(Principal, String), Box<dyn Fn(Vec<u8>) -> ()>>,
    handler: Option<Box<dyn Fn(Vec<u8>) -> ()>>
}

pub trait De {
    fn from_vec(_: Vec<u8>) -> Self;
}

#[derive(Debug)]
struct X {
    i: u32
}

impl De for X {
    fn from_vec(v: Vec<u8>) -> Self {
        println!("From Vec {:?}", v);
        X {
            i: 17
        }
    }
}

impl TestContext {
    pub fn handler<P: De, T: 'static + Fn(P) -> ()>(&mut self, cb: T) {
        self.handler = Some(Box::new(move |x| {
            println!("XXX");
            cb(P::from_vec(x));
        }));
    }

    pub fn call(&self, v: Vec<u8>) {
        (*self.handler.as_ref().unwrap())(v);
    }
}

// pub fn get_context() -> &'static mut TestContext {
//     &mut TestContext {}
// }

#[test]
fn xxx() {
    let mut ctx = TestContext {
        handlers: BTreeMap::new(),
        handler: None
    };

    ctx.handler(|v: X| {
        println!("Called {:?}", v);
    });

    ctx.call(vec![0, 1]);
}
