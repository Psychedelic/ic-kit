#[cfg(not(target_arch = "wasm32"))]
thread_local!(static HANDLER: std::cell::RefCell<Option<Box<dyn Ic0CallHandler>>> = std::cell::RefCell::new(None));

/// Register a handler to be used for handling the canister call in non-wasm environments.
///
/// # Panics
///
/// If called from within a canister.
pub fn register_handler<H: Ic0CallHandler + 'static>(handler: H) {
    #[cfg(not(target_arch = "wasm32"))]
    HANDLER.with(|c| {
        let _ = c.borrow_mut().insert(Box::new(handler));
    });

    #[cfg(target_arch = "wasm32")]
    {
        let _ = handler;
        panic!("This method is not usable inside the canister.")
    }
}

macro_rules! _ic0_module_ret {
    ( ( $_: ident : $t: ty ) ) => {
        $t
    };
    ( ( $_i1: ident : $t1: ty , $_i2: ident : $t2: ty) ) => {
        ($t1, $t2)
    };
    ( ( $t: ty ) ) => {
        $t
    };
    ( $t: ty ) => {
        $t
    };
}

macro_rules! ic0_module {
    ( $(     ic0. $name: ident : ( $( $argname: ident : $argtype: ty ),* ) -> $rettype: tt ; )+ ) => {
        #[allow(improper_ctypes)]
        #[cfg(target_arch = "wasm32")]
        #[link(wasm_import_module = "ic0")]
        extern "C" {
            $(pub fn $name($( $argname: $argtype, )*) -> _ic0_module_ret!($rettype) ;)*
        }

        /// An object that implements mock handlers for ic0 WASM API calls.
        pub trait Ic0CallHandler {
            $(
            fn $name(&mut self, $($argname: $argtype,)*) -> _ic0_module_ret!($rettype);
            )*
        }

        /// The runtime module provides the tools to have the canister in one thread and communicate
        /// with another handler on another thread.
        #[cfg(feature = "runtime")]
        pub mod runtime {
            use futures::executor::block_on;
            use super::Ic0CallHandler;

            /// A response from the runtime to the canister.
            #[derive(Debug)]
            pub enum Response {
                None,
                Isize(isize),
                I32(i32),
                I64(i64),
                Trap,
            }

            impl From<()> for Response {
                #[inline(always)]
                fn from(_: ()) -> Self {
                    Response::None
                }
            }

            impl Into<()> for Response {
                #[inline(always)]
                fn into(self) -> () {
                    match self {
                        Response::None => (),
                        Response::Trap => panic!("Canister trapped."),
                        _ => panic!("unexpected type cast."),
                    }
                }
            }

            impl From<isize> for Response {
                #[inline(always)]
                fn from(n: isize) -> Self {
                    Response::Isize(n)
                }
            }

            impl Into<isize> for Response {
                #[inline(always)]
                fn into(self) -> isize {
                    match self {
                        Response::Isize(n) => n,
                        Response::Trap => panic!("Canister trapped."),
                        _ => panic!("unexpected type cast."),
                    }
                }
            }

            impl From<i32> for Response {
                #[inline(always)]
                fn from(n: i32) -> Self {
                    Response::I32(n)
                }
            }

            impl Into<i32> for Response {
                #[inline(always)]
                fn into(self) -> i32 {
                    match self {
                        Response::I32(n) => n,
                        Response::Trap => panic!("Canister trapped."),
                        _ => panic!("unexpected type cast."),
                    }
                }
            }

            impl From<i64> for Response {
                #[inline(always)]
                fn from(n: i64) -> Self {
                    Response::I64(n)
                }
            }

            impl Into<i64> for Response {
                #[inline(always)]
                fn into(self) -> i64 {
                    match self {
                        Response::I64(n) => n,
                        Response::Trap => panic!("Canister trapped."),
                        _ => panic!("unexpected type cast."),
                    }
                }
            }

            /// A request from the canister to the handler.
            #[derive(Debug)]
            pub enum Request {
                $(
                $name {
                    $($argname: $argtype,)*
                },
                )*
            }

            impl Request {
                #[inline(always)]
                pub fn proxy<H: Ic0CallHandler>(self, handler: &mut H) -> Response {
                    match self {
                        $(
                        Request::$name { $($argname,)* } => handler.$name($($argname,)*).into(),
                        )*
                    }
                }
            }

            pub struct RuntimeHandle {
                rx: tokio::sync::mpsc::Receiver<Response>,
                tx: tokio::sync::mpsc::Sender<Request>,
            }

            impl RuntimeHandle {
                pub fn new(
                    rx: tokio::sync::mpsc::Receiver<Response>,
                    tx: tokio::sync::mpsc::Sender<Request>,
                ) -> Self {
                    Self {
                        rx,
                        tx
                    }
                }
            }

            impl Ic0CallHandler for RuntimeHandle {
                $(
                fn $name(&mut self, $($argname: $argtype,)*) -> _ic0_module_ret!($rettype) {
                    block_on(async {
                        self.tx
                            .send(Request::$name {$($argname,)*})
                            .await
                            .expect("ic-kit-runtime: Failed to send message from canister thread.");
                        self.rx.recv().await.expect("Channel closed").into()
                    })
                }
                )*
            }
        }

        $(
        #[cfg(not(target_arch = "wasm32"))]
        pub unsafe fn $name($( $argname: $argtype, )*) -> _ic0_module_ret!($rettype) {
            HANDLER.with(|handler| {
                std::cell::RefMut::map(handler.borrow_mut(), |h| {
                    h.as_mut().expect("No handler set for current thread.")
                })
                .$name($( $argname, )*)
            })
        }
        )*
    };
}

// Copy-paste the spec section of the API here.
// https://github.com/dfinity/interface-spec/blob/master/spec/ic0.txt
//
// The comment after each function lists from where these functions may be invoked:
// I: from canister_init or canister_post_upgrade
// G: from canister_pre_upgrade
// U: from canister_update …
// Q: from canister_query …
// Ry: from a reply callback
// Rt: from a reject callback
// C: from a cleanup callback
// s: the (start) module initialization function
// F: from canister_inspect_message
// H: from canister_heartbeat
// * = I G U Q Ry Rt C F H (NB: Not (start))
ic0_module! {
    ic0.msg_arg_data_size : () -> isize;                                               // I U Q Ry F
    ic0.msg_arg_data_copy : (dst : isize, offset : isize, size : isize) -> ();         // I U Q Ry F
    ic0.msg_caller_size : () -> isize;                                                 // I G U Q F
    ic0.msg_caller_copy : (dst : isize, offset: isize, size : isize) -> ();            // I G U Q F
    ic0.msg_reject_code : () -> i32;                                                   // Ry Rt
    ic0.msg_reject_msg_size : () -> isize;                                             // Rt
    ic0.msg_reject_msg_copy : (dst : isize, offset : isize, size : isize) -> ();       // Rt

    ic0.msg_reply_data_append : (src : isize, size : isize) -> ();                     // U Q Ry Rt
    ic0.msg_reply : () -> ();                                                          // U Q Ry Rt
    ic0.msg_reject : (src : isize, size : isize) -> ();                                // U Q Ry Rt

    ic0.msg_cycles_available : () -> i64;                                              // U Rt Ry
    ic0.msg_cycles_available128 : (dst : isize) -> ();                                 // U Rt Ry
    ic0.msg_cycles_refunded : () -> i64;                                               // Rt Ry
    ic0.msg_cycles_refunded128 : (dst : isize) -> ();                                  // Rt Ry
    ic0.msg_cycles_accept : (max_amount : i64) -> (amount : i64);                      // U Rt Ry
    ic0.msg_cycles_accept128 : (max_amount_high : i64, max_amount_low: i64, dst : isize)
                           -> ();                                                      // U Rt Ry

    ic0.canister_self_size : () -> isize;                                              // *
    ic0.canister_self_copy : (dst : isize, offset : isize, size : isize) -> ();        // *
    ic0.canister_cycle_balance : () -> i64;                                            // *
    ic0.canister_cycle_balance128 : (dst : isize) -> ();                               // *
    ic0.canister_status : () -> i32;                                                   // *

    ic0.msg_method_name_size : () -> isize;                                            // F
    ic0.msg_method_name_copy : (dst : isize, offset : isize, size : isize) -> ();      // F
    ic0.accept_message : () -> ();                                                     // F

    ic0.call_new :                                                                     // U Ry Rt H
      ( callee_src  : isize,
        callee_size : isize,
        name_src : isize,
        name_size : isize,
        reply_fun : isize,
        reply_env : isize,
        reject_fun : isize,
        reject_env : isize
      ) -> ();
    ic0.call_on_cleanup : (fun : isize, env : isize) -> ();                            // U Ry Rt H
    ic0.call_data_append : (src : isize, size : isize) -> ();                          // U Ry Rt H
    ic0.call_cycles_add : (amount : i64) -> ();                                        // U Ry Rt H
    ic0.call_cycles_add128 : (amount_high : i64, amount_low: i64) -> ();               // U Ry Rt H
    ic0.call_perform : () -> ( err_code : i32 );                                       // U Ry Rt H

    ic0.stable_size : () -> (page_count : i32);                                        // *
    ic0.stable_grow : (new_pages : i32) -> (old_page_count : i32);                     // *
    ic0.stable_write : (offset : i32, src : isize, size : isize) -> ();                // *
    ic0.stable_read : (dst : isize, offset : i32, size : isize) -> ();                 // *
    ic0.stable64_size : () -> (page_count : i64);                                      // *
    ic0.stable64_grow : (new_pages : i64) -> (old_page_count : i64);                   // *
    ic0.stable64_write : (offset : i64, src : i64, size : i64) -> ();                  // *
    ic0.stable64_read : (dst : i64, offset : i64, size : i64) -> ();                   // *

    ic0.certified_data_set : (src: isize, size: isize) -> ();                          // I G U Ry Rt H
    ic0.data_certificate_present : () -> i32;                                          // *
    ic0.data_certificate_size : () -> isize;                                           // *
    ic0.data_certificate_copy : (dst: isize, offset: isize, size: isize) -> ();        // *

    ic0.time : () -> (timestamp : i64);                                                // *
    ic0.performance_counter : (counter_type : i32) -> (counter : i64);                 // * s

    ic0.debug_print : (src : isize, size : isize) -> ();                               // * s
    ic0.trap : (src : isize, size : isize) -> ();                                      // * s
}
