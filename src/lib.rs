mod inject;
mod interface;
mod mock;

pub use interface::*;
pub use mock::*;

#[inline]
pub fn get_context() -> &'static mut impl Context {
    #[cfg(not(target_family = "wasm"))]
    inject::get_context()
}

fn x() {
    let ic = get_context();

    println!("Cycles {}", ic.cycles_available());
    ic.cycles_accept(10);
    println!("Cycles {}", ic.cycles_available());
}

#[test]
fn with_1000_cycles() {
    MockContext::new().with_cycles(100).inject();
    x();
}

#[test]
fn with_no_cycles() {
    MockContext::new().inject();
    x();
}
