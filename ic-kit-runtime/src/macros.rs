#[macro_export]
macro_rules! canister_builder {
    ( $name: ident { $( $method: ident),+ } ) => {
        pub struct $name;

        impl $name {
            /// Create a new instance of this canister.
            pub fn new(canister_id: ic_kit::Principal) -> ic_kit::rt::Canister {
                ic_kit::rt::Canister::new(canister_id)
                    $(.with_method::<$method>())*
            }

            /// Create a new instance of this canister with an anonymous id.
            pub fn anonymous() -> ic_kit::rt::Canister {
                Self::new(ic_kit::Principal::anonymous())
            }
        }
    };
}
