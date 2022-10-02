use std::sync::Mutex;

use lazy_static::lazy_static;
use proc_macro2::TokenStream;
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use syn::{Error, spanned::Spanned};

struct Handler {
    name: String,
    route: String,
    method: String,
    upgrade: bool,
}

lazy_static! {
    static ref HANDLERS: Mutex<Vec<Handler>> = Mutex::new(Vec::new());
}

#[derive(Deserialize)]
struct Config {
    route: String,
    upgrade: Option<bool>,
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

    HANDLERS.lock().unwrap().push(Handler {
        name: fun.sig.ident.to_string(),
        route: attrs.route,
        method: method.into(),
        upgrade: attrs.upgrade.unwrap_or(false),
    });

    Ok(quote! {
        #item
    })
}

pub fn gen_http_request_code() -> TokenStream {
    let routes = HANDLERS.lock().unwrap();

    let mut routes_insert = TokenStream::new();
    let mut upgradable = false;

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

            router.insert("GET", "/*p", (index, false));
        };
    } else {
        for Handler {
            method,
            name,
            route,
            upgrade,
        } in routes.iter()
        {
            let name = syn::Ident::new(name, proc_macro2::Span::call_site());

            if *upgrade {
                upgradable = true;
            }

            routes_insert.extend(quote! {
                router.insert(#method, #route, (#name, #upgrade));
            });
        }
    }

    let mut upgrade_code = TokenStream::new();
    let mut query_code = TokenStream::new();

    if upgradable {
        query_code.extend(quote! {
            let (handler, upgrade) = m.value;
            if *upgrade {
                ic_kit::http::HttpResponse {
                    status_code: 100,
                    headers: vec![],
                    body: vec![],
                    streaming_strategy: None,
                    upgrade: true,
                }
            } else {
                handler(req, m.params)
            }
        });

        upgrade_code.extend(quote! {
            #[doc(hidden)]
            #[export_name = "canister_update http_request_update"]
            fn _ic_kit_canister_update_http_request_update() {
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
                    let result = match router.at(&req.method.clone(), &req.url.clone()) {
                        Ok(m) => {
                            let (handler, _) = m.value;
                            handler(req, m.params)
                        },
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
        });
    } else {
        query_code.extend(quote! {
            let (handler, _) = m.value;
            handler(req, m.params)
        });
    };

    quote! {
        pub type HandlerFn = (fn(ic_kit::http::HttpRequest, ic_kit::http::Params) -> ic_kit::http::HttpResponse, bool);

        #[derive(Clone)]
        pub struct Router {
            get: ic_kit::http::BasicRouter<HandlerFn>,
            post: ic_kit::http::BasicRouter<HandlerFn>,
            put: ic_kit::http::BasicRouter<HandlerFn>,
            delete: ic_kit::http::BasicRouter<HandlerFn>,
            patch: ic_kit::http::BasicRouter<HandlerFn>,
            head: ic_kit::http::BasicRouter<HandlerFn>,
            options: ic_kit::http::BasicRouter<HandlerFn>,
        }

        impl Default for Router {
            fn default() -> Self {
                let mut router = Self {
                    get: ic_kit::http::BasicRouter::new(),
                    post: ic_kit::http::BasicRouter::new(),
                    put: ic_kit::http::BasicRouter::new(),
                    delete: ic_kit::http::BasicRouter::new(),
                    patch: ic_kit::http::BasicRouter::new(),
                    head: ic_kit::http::BasicRouter::new(),
                    options: ic_kit::http::BasicRouter::new(),
                };
                #routes_insert
                router
            }
        }

        impl Router {
            pub fn insert(&mut self, method: &str, path: &str, handler: HandlerFn) {
                match method {
                    "GET" => self.get.insert(path, handler).unwrap(),
                    "POST" => self.post.insert(path, handler).unwrap(),
                    "PUT" => self.put.insert(path, handler).unwrap(),
                    "DELETE" => self.delete.insert(path, handler).unwrap(),
                    "PATCH" => self.patch.insert(path, handler).unwrap(),
                    "HEAD" => self.head.insert(path, handler).unwrap(),
                    "OPTIONS" => self.options.insert(path, handler).unwrap(),
                    _ => panic!("unsupported method: {}", method),
                };
            }

            pub fn at<'s: 'p, 'p>(
                &'s self,
                method: &str,
                path: &'p str,
            ) -> Result<Match<'s, 'p, &HandlerFn>, MatchError> {
                match method {
                    "GET" => self.get.at(path),
                    "POST" => self.post.at(path),
                    "PUT" => self.put.at(path),
                    "DELETE" => self.delete.at(path),
                    "PATCH" => self.patch.at(path),
                    "HEAD" => self.head.at(path),
                    "OPTIONS" => self.options.at(path),
                    _ => Err(MatchError::NotFound),
                }
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
                let result = match router.at(&req.method.clone(), &req.url.clone()) {
                    Ok(m) => {
                        #query_code
                    },
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

        #upgrade_code
    }
}
