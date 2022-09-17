use entry::{gen_entry_point_code, EntryPoint};
use proc_macro::TokenStream;
use syn::parse_macro_input;
use test::gen_test_code;

mod entry;
mod export_service;
mod test;

#[cfg(feature = "http")]
mod http;

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

#[proc_macro_derive(KitCanister, attributes(candid_path))]
pub fn kit_export(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let save_candid_path_result = get_save_candid_path(&input);

    match save_candid_path_result {
        Ok(save_candid_path) => export_service::export_service(input, save_candid_path).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn get_save_candid_path(input: &syn::DeriveInput) -> syn::Result<Option<syn::LitStr>> {
    let candid_path_helper_attribute_option = input
        .attrs
        .iter()
        .find(|attr| attr.path.is_ident("candid_path"));

    match candid_path_helper_attribute_option {
        Some(candid_path_helper_attribute) => {
            let custom_candid_path_lit: syn::LitStr = candid_path_helper_attribute.parse_args()?;
            Ok(Some(custom_candid_path_lit))
        }
        None => Ok(None),
    }
}

/// Export a function as a HTTP GET handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("GET", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP POST handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("POST", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP PUT handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("PUT", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP DELETE handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn delete(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("DELETE", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP PATCH handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn patch(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("PATCH", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP OPTIONS handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn options(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("OPTIONS", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP HEAD handler.
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn head(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("HEAD", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
