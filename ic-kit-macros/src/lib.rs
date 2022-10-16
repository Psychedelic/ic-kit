mod entry;
mod export_service;
mod test;

use entry::{gen_entry_point_code, EntryPoint};
use proc_macro::TokenStream;
use syn::parse_macro_input;
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

#[proc_macro_derive(KitCanister, attributes(candid_path, wasm_path))]
pub fn kit_export(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);

    let save_candid_path = match get_attribute_lit(&input, "candid_path") {
        Ok(attr) => attr,
        Err(e) => return e.to_compile_error().into(),
    };

    let wasm_path = match get_attribute_lit(&input, "wasm_path") {
        Ok(attr) => attr,
        Err(e) => return e.to_compile_error().into(),
    };

    let config = export_service::ExportServiceConfig {
        name: input.ident,
        save_candid_path,
        wasm_path,
    };

    export_service::export_service(config).into()
}

fn get_attribute_lit(
    input: &syn::DeriveInput,
    attr_name: &str,
) -> syn::Result<Option<syn::LitStr>> {
    input
        .attrs
        .iter()
        .find(|attr| attr.path.is_ident(attr_name))
        .map(|attr| attr.parse_args::<syn::LitStr>())
        .map_or(Ok(None), |e| e.map(Some))
}
