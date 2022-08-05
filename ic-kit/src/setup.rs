use crate::ic;
use std::panic;

static mut DONE: bool = false;

pub fn setup_hooks() {
    unsafe {
        if DONE {
            return;
        }
        DONE = true;
    }

    set_panic_hook();
}

/// Sets a custom panic hook, uses debug.trace
pub fn set_panic_hook() {
    panic::set_hook(Box::new(|info| {
        let file = info.location().unwrap().file();
        let line = info.location().unwrap().line();
        let col = info.location().unwrap().column();

        let msg = match info.payload().downcast_ref::<&'static str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => &s[..],
                None => "Box<Any>",
            },
        };

        let err_info = format!("Panicked at '{}', {}:{}:{}", msg, file, line, col);
        ic::print(&err_info);
        ic::trap(&err_info);
    }));
}
