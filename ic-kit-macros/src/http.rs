use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use std::sync::Mutex;
use syn::{spanned::Spanned, Error};

struct Method {
    name: String,
    route: String,
    method: String,
}

lazy_static! {
    static ref GETS: Mutex<Vec<Method>> = Mutex::new(Vec::new());
}

#[derive(Deserialize)]
struct Config {
    route: String,
}

/// Process a rust syntax and generate the code for processing it.
pub fn gen_handler_code(
    method: &str,
    attr: TokenStream,
    item: TokenStream,
) -> Result<TokenStream, Error> {
    let attrs = from_tokenstream::<Config>(&attr)?;
    let fun: syn::ItemFn = syn::parse2::<syn::ItemFn>(item.clone()).map_err(|e| {
        Error::new(
            item.span(),
            format!("#[{0}] must be above a function. \n{1}", method, e),
        )
    })?;

    GETS.lock().unwrap().push(Method {
        name: fun.sig.ident.to_string(),
        route: attrs.route,
        method: method.into(),
    });

    Ok(quote! {
        #item
    })
}

pub fn gen_http_request_code() -> TokenStream {
    let routes = GETS.lock().unwrap();

    let mut routes_insert = TokenStream::new();

    if routes.is_empty() {
        // if no routes, provide a basic index displaying canister stats
        routes_insert = quote! {
            fn index(_: ic_kit::http::HttpRequest, _: ic_kit::http::Params) -> ic_kit::http::HttpResponse {
                let res = format!("{{\"cycles\": {}}}", ic_kit::ic::balance());

                ic_kit::http::HttpResponse {
                    status_code: 200,
                    headers: vec![],
                    body: res.into_bytes(),
                    streaming_strategy: None,
                    upgrade: false,
                }
            }

            router.insert("/", &index);
        };
    } else {
        for Method {
            method,
            name,
            route,
        } in routes.iter()
        {
            let name = syn::Ident::new(name, proc_macro2::Span::call_site());

            routes_insert.extend(quote! {
                router.0.insert(#route, &#name);
            });
        }
    }
    quote! {
        pub type HandlerFn = dyn Fn(ic_kit::http::HttpRequest, ic_kit::http::Params) -> ic_kit::http::HttpResponse;


        #[derive(Clone)]
        pub struct Router<'a>(ic_kit::http::BasicRouter<&'a HandlerFn>);
        impl<'a> Router<'a> {
            pub fn insert(&mut self, path: &str, handler: &'a HandlerFn) {
                self.0
                    .insert(path, handler)
                    .unwrap_or_else(|e| ic::trap(&format!("{}", e)));
            }
        }


        impl<'a> Default for Router<'a> {
            fn default() -> Self {
                let mut router = Self(ic_kit::http::BasicRouter::new());
                #routes_insert
                router
            }
        }

        #[doc(hidden)]
        #[export_name = "canister_query http_request"]
        fn _ic_kit_canister_query_http_request() {
            let bytes = ic_kit::utils::arg_data_raw();
            let args: (ic_kit::http::HttpRequest,) = match ic_kit::candid::decode_args(&bytes) {
                Ok(v) => v,
                Err(_) => {
                    ic_kit::utils::reject("Could not decode arguments.");
                    return;
                }
            };
            let (req,) = args;
            ic_kit::ic::with(|router: &Router| {
                // let certificate = ic::data_certificate().unwrap_or_else(|| ic::trap("no data certificate available"));
                // ic::print(format!("{:?} {:?}", req, certificate));
                let result = match router.0.at(req.url.clone().as_str()) {
                    Ok(m) => (m.value)(req, m.params),
                    Err(e) => ic_kit::http::HttpResponse {
                        status_code: 404,
                        headers: vec![],
                        body: e.to_string().as_bytes().to_vec(),
                        streaming_strategy: None,
                        upgrade: false,
                    },
                };
                let bytes =
                    ic_kit::candid::encode_one(result).expect("Could not encode canister's response.");
                ic_kit::utils::reply(&bytes);
            });
        }

    }
}
