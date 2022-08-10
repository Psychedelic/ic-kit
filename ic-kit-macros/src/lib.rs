mod entry;
mod test;

use entry::{gen_entry_point_code, EntryPoint};
use proc_macro::TokenStream;
use test::gen_test_code;

fn process_entry_point(
    entry_point: EntryPoint,
    attr: TokenStream,
    item: TokenStream,
) -> TokenStream {
    gen_entry_point_code(entry_point, attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export the function as the init hook of the canister.
#[proc_macro_attribute]
pub fn init(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::Init, attr, item)
}

/// Export the function as the pre_upgrade hook of the canister.
#[proc_macro_attribute]
pub fn pre_upgrade(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::PreUpgrade, attr, item)
}

/// Export the function as the post_upgrade hook of the canister.
#[proc_macro_attribute]
pub fn post_upgrade(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::PostUpgrade, attr, item)
}

/// Export the function as the inspect_message hook of the canister.
#[proc_macro_attribute]
pub fn inspect_message(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::InspectMessage, attr, item)
}

/// Export the function as the heartbeat hook of the canister.
#[proc_macro_attribute]
pub fn heartbeat(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::Heartbeat, attr, item)
}

/// Export an update method for the canister.
#[proc_macro_attribute]
pub fn update(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::Update, attr, item)
}

/// Export a query method for the canister.
#[proc_macro_attribute]
pub fn query(attr: TokenStream, item: TokenStream) -> TokenStream {
    process_entry_point(EntryPoint::Query, attr, item)
}

/// A macro to generate IC-Kit tests.
#[proc_macro_attribute]
pub fn kit_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    gen_test_code(attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
