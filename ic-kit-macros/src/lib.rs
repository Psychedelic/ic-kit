use proc_macro::TokenStream;

use syn::parse_macro_input;

use entry::{gen_entry_point_code, EntryPoint};
use test::gen_test_code;

mod entry;
mod export_service;

#[cfg(feature = "http")]
mod http;

mod di;
mod metadata;
mod test;

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
///
/// The function must have the signature
/// > fn([HttpRequest](../ic_kit_http/struct.HttpRequest.html), [Params](../ic_kit_http/struct.Params.html)) -> [HttpResponse](../ic_kit_http/struct.HttpResponse.html)
///
/// HTTP macros will remove dependency injected reference args from the function signature, so you can use DI in your handlers.
///
/// # Example
/// ```rs
/// // set a route for GET / that has no params
/// #[get(route = "/")]
/// fn index_handler(r: HttpRequest, _: Params) -> HttpResponse {
///     ic::print(format!("{:?}", r));
///
///     // grab a header
///     let header = r.headers.get("host").unwrap();
///
///     // Build an Ok (200) response with a body containing the host header
///     HttpResponse::ok().body(header)
/// }
/// ```
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn get(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("GET", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP POST handler.
///
/// The function must have the signature
/// > fn([HttpRequest](../ic_kit_http/struct.HttpRequest.html), [Params](../ic_kit_http/struct.Params.html)) -> [HttpResponse](../ic_kit_http/struct.HttpResponse.html)
///
/// HTTP macros will remove dependency injected reference args from the function signature, so you can use DI in your handlers.
///
/// # Example
/// Store a value in the canister's memory, with the key being a path parameter.
///
/// ```rs
/// pub type Data = HashMap<String, Vec<u8>>;
///
/// // set a route for POST /data/<key> that has a single path param, and upgrades to an update call
/// #[post(route = "/set/:key", upgrade = true)]
/// fn set_handler(r: HttpRequest, p: Params) -> HttpResponse {
///    let key = p.get("key").unwrap();
///    let value = r.body;
///
///    ic_kit::with_mut(|data: &mut Data| {
///      data.insert(key, value);
///    });
///    
///    HttpResponse::ok().body("stored value")
/// }
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn post(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("POST", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP PUT handler.
///
/// The function must have the signature
/// > fn([HttpRequest](../ic_kit_http/struct.HttpRequest.html), [Params](../ic_kit_http/struct.Params.html)) -> [HttpResponse](../ic_kit_http/struct.HttpResponse.html)
///
/// HTTP macros will remove dependency injected reference args from the function signature, so you can use DI in your handlers.
///
/// # Example
///
/// ```rs
/// pub type Data = HashMap<String, Vec<u8>>;
///
/// // set a route for PUT /<filename> that has a single path param, and upgrades to an update call
/// #[put(route = ":filename", upgrade = true)]
/// fn put_handler(data: &mut Data, r: HttpRequest, p: Params) -> HttpResponse {
///    // get the filename param
///    let file = p.get("filename").unwrap();
///    let value = r.body;
///    
///    data.insert(file, value);
///    
///    // Build an Ok (200) response with a body
///    HttpResponse::ok().body("stored file")
/// }
/// ```
#[cfg(feature = "http")]
#[proc_macro_attribute]
pub fn put(attr: TokenStream, item: TokenStream) -> TokenStream {
    http::gen_handler_code("PUT", attr.into(), item.into())
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Export a function as a HTTP DELETE handler.
///
/// The function must have the signature
/// > fn([HttpRequest](../ic_kit_http/struct.HttpRequest.html), [Params](../ic_kit_http/struct.Params.html)) -> [HttpResponse](../ic_kit_http/struct.HttpResponse.html)
///
/// HTTP macros will remove dependency injected reference args from the function signature, so you can use DI in your handlers.
///
/// # Example
///
/// ```rs
/// pub type Data = HashMap<String, Vec<u8>>;
///
/// // set a route for DELETE /<filename> that has a single path param, and upgrades to an update call
/// #[delete(route = ":file", upgrade = true)]
/// fn delete_handler(data: &mut Data, r: HttpRequest, p: Params) -> HttpResponse {
///   let file = p.get("file").unwrap();
///
///   data.remove(file);
///
///   // Build an Ok (200) response with a body
///   HttpResponse::ok().body("deleted file")
/// }
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
