#[macro_export]
macro_rules! canister_builder {
    ( $name: ident { $( $method: ident)+ } ) => {
        #[cfg(not(target_family = "wasm"))]
        pub struct $name;

        #[cfg(not(target_family = "wasm"))]
        impl $name {
            pub fn new(canister_id: ic_kit::Principal) -> ic_kit::rt::Canister {
                ic_kit::rt::Canister::new(canister_id)
                    $(.with_method::<$method>())*
            }
        }
    };
}
