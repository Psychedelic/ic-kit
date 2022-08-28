//! Generate the Rust code for Internet Computer's [entry points] [1]
//!
//! [1]: <https://internetcomputer.org/docs/current/references/ic-interface-spec/#entry-points>

use crate::export_service::declare;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use std::fmt::Formatter;
use syn::{spanned::Spanned, Error};

#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq)]
pub enum EntryPoint {
    Init,
    PreUpgrade,
    PostUpgrade,
    InspectMessage,
    Heartbeat,
    Update,
    Query,
}

impl std::fmt::Display for EntryPoint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryPoint::Init => f.write_str("init"),
            EntryPoint::PreUpgrade => f.write_str("pre_upgrade"),
            EntryPoint::PostUpgrade => f.write_str("post_upgrade"),
            EntryPoint::InspectMessage => f.write_str("inspect_message"),
            EntryPoint::Heartbeat => f.write_str("heartbeat"),
            EntryPoint::Update => f.write_str("update"),
            EntryPoint::Query => f.write_str("query"),
        }
    }
}

impl EntryPoint {
    pub fn is_lifecycle(&self) -> bool {
        match &self {
            EntryPoint::Update | EntryPoint::Query => false,
            _ => true,
        }
    }

    pub fn is_inspect_message(&self) -> bool {
        match &self {
            EntryPoint::InspectMessage => true,
            _ => false,
        }
    }
}

#[derive(Deserialize)]
struct Config {
    name: Option<String>,
    guard: Option<String>,
}

/// Process a rust syntax and generate the code for processing it.
pub fn gen_entry_point_code(
    entry_point: EntryPoint,
    attr: TokenStream,
    item: TokenStream,
) -> Result<TokenStream, Error> {
    let attrs = from_tokenstream::<Config>(&attr)?;
    let fun: syn::ItemFn = syn::parse2::<syn::ItemFn>(item.clone()).map_err(|e| {
        Error::new(
            item.span(),
            format!("#[{0}] must be above a function. \n{1}", entry_point, e),
        )
    })?;
    let signature = &fun.sig;
    let visibility = &fun.vis;
    let generics = &signature.generics;
    let is_async = signature.asyncness.is_some();
    let name = &signature.ident;

    let return_length = match &signature.output {
        syn::ReturnType::Default => 0,
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            syn::Type::Tuple(tuple) => tuple.elems.len(),
            _ => 1,
        },
    };

    if is_async && !generics.params.is_empty() {
        return Err(Error::new(
            generics.span(),
            format!(
                "#[{}] must be above a async function with no generic parameters.",
                entry_point
            ),
        ));
    }

    if entry_point.is_lifecycle() && !entry_point.is_inspect_message() && return_length > 0 {
        return Err(Error::new(
            signature.output.span(),
            format!("#[{}] function cannot have a return value.", entry_point),
        ));
    }

    if entry_point.is_lifecycle() && attrs.name.is_some() {
        return Err(Error::new(
            Span::call_site(),
            format!("#[{}] function cannot be renamed.", entry_point),
        ));
    }

    if entry_point.is_inspect_message() && return_length != 1 {
        return Err(Error::new(
            Span::call_site(),
            format!(
                "#[{}] function must have a boolean return value.",
                entry_point
            ),
        ));
    }

    if entry_point.is_lifecycle() && attrs.guard.is_some() {
        return Err(Error::new(
            Span::call_site(),
            format!("#[{}] function cannot have a guard", entry_point),
        ));
    }

    if entry_point.is_lifecycle() && is_async {
        return Err(Error::new(
            Span::call_site(),
            format!("#[{}] function cannot be async.", entry_point),
        ));
    }

    let outer_function_ident = Ident::new(
        &format!("_ic_kit_canister_{}_{}", entry_point, name),
        Span::call_site(),
    );

    let guard = if let Some(guard_name) = attrs.guard {
        let guard_ident = Ident::new(&guard_name, Span::call_site());

        quote! {
            let r: Result<(), String> = #guard_ident ();
            if let Err(e) = r {
                ic_kit::utils::reject(&e);
                return;
            }
        }
    } else {
        quote! {}
    };

    let candid_name = attrs.name.unwrap_or_else(|| name.to_string());
    let export_name = if entry_point.is_lifecycle() {
        format!("canister_{}", entry_point)
    } else {
        format!("canister_{0} {1}", entry_point, candid_name)
    };

    // Build the outer function's body.
    let tmp = di(collect_args(entry_point, signature)?, is_async)?;
    let args = tmp.args;
    let (can_args, can_types): (Vec<_>, Vec<_>) = tmp.can_args.into_iter().unzip();
    let (imu_args, imu_types): (Vec<_>, Vec<_>) = tmp.imu_args.into_iter().unzip();
    let (mut_args, mut_types): (Vec<_>, Vec<_>) = tmp.mut_args.into_iter().unzip();

    // If the method does not accept any arguments, don't even read the msg_data, and if the
    // deserialization fails, just reject the message, which is cheaper than trap.
    let arg_decode = if can_args.len() == 0 {
        quote! {}
    } else {
        quote! {
            let bytes = ic_kit::utils::arg_data_raw();
            let args = match ic_kit::candid::decode_args(&bytes) {
                Ok(v) => v,
                Err(_) => {
                    ic_kit::utils::reject("Could not decode arguments.");
                    return;
                },
            };
            let ( #( #can_args, )* ) = args;
        }
    };

    let return_encode = if entry_point.is_inspect_message() {
        quote! {
            let result: bool = result;
            if result == true {
                ic_kit::utils::accept();
            }
        }
    } else if entry_point.is_lifecycle() {
        quote! {}
    } else {
        match return_length {
            0 => quote! {
                // Send the precomputed `encode_args(())` available in ic-kit.
                let _ = result; // to ignore result not being used.
                ic_kit::utils::reply(ic_kit::ic::CANDID_EMPTY_ARG)
            },
            1 => quote! {
                let bytes = ic_kit::candid::encode_one(result)
                    .expect("Could not encode canister's response.");
                ic_kit::utils::reply(&bytes);
            },
            _ => quote! {
                let bytes = ic_kit::candid::encode_args(result)
                    .expect("Could not encode canister's response.");
                ic_kit::utils::reply(&bytes);
            },
        }
    };

    // Because DI doesn't work on an async method.
    let mut sync_result = quote! {
        let result = #name ( #(#args),* );
        #return_encode
    };

    sync_result = match imu_args.len() {
        0 => sync_result,
        1 => quote! {
            ic_kit::ic::with(|#(#imu_args: &#imu_types),*| {
                #sync_result
            });
        },
        _ => quote! {
            ic_kit::ic::with_many(|(#(#imu_args),*) : (#(&#imu_types),*)| {
                #sync_result
            });
        },
    };

    sync_result = match mut_args.len() {
        0 => sync_result,
        1 => quote! {
            ic_kit::ic::with_mut(|#(#mut_args: &mut #mut_types),*| {
                #sync_result
            });
        },
        _ => quote! {
            ic_kit::ic::with_many_mut(|(#(#mut_args),*) : (#(&mut #mut_types),*)| {
                #sync_result
            });
        },
    };

    // only spawn for async methods.
    let body = if is_async {
        quote! {
            ic_kit::ic::spawn(async {
                #arg_decode
                let result = #name ( #(#args),* ).await;
                #return_encode
            });
        }
    } else {
        quote! {
            #arg_decode
            #sync_result;
        }
    };

    declare(
        entry_point,
        name.clone(),
        candid_name,
        can_args,
        can_types,
        &signature.output,
    )?;

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #[cfg(not(target_family = "wasm"))]
        #visibility struct #name {}

        #[cfg(not(target_family = "wasm"))]
        impl ic_kit::rt::CanisterMethod for #name {
            const EXPORT_NAME: &'static str = #export_name;

            fn exported_method() {
                #outer_function_ident()
            }
        }

        #[doc(hidden)]
        #[export_name = #export_name]
        fn #outer_function_ident() {
            #[cfg(target_family = "wasm")]
            ic_kit::setup_hooks();

            #guard
            #body
        }

        #[inline(always)]
        #item
    })
}

#[derive(Default)]
struct ProcessedArgs {
    args: Vec<Ident>,
    mut_args: Vec<(Ident, syn::Type)>,
    imu_args: Vec<(Ident, syn::Type)>,
    can_args: Vec<(Ident, syn::Type)>,
    injected: Vec<syn::Type>,
}

fn di(args: Vec<(Ident, syn::Type)>, is_async: bool) -> Result<ProcessedArgs, Error> {
    let mut result = ProcessedArgs::default();

    for (ident, ty) in args {
        result.args.push(ident.clone());

        match ty {
            syn::Type::Reference(ty_ref) if is_async => {
                return Err(Error::new(
                    ty_ref.span(),
                    format!("IC-Kit's dependency injection can only work on sync methods."),
                ));
            }
            syn::Type::Reference(ty_ref) if !result.can_args.is_empty() => {
                return Err(Error::new(
                    ty_ref.span(),
                    format!("An IC-kit dependency injected reference could only come before canister arguments."),
                ));
            }
            syn::Type::Reference(ty_ref) if result.injected.contains(&ty_ref.elem) => {
                return Err(Error::new(
                    ty_ref.span(),
                    format!(
                        "IC-Kit's dependency injection can only inject one instance of each type."
                    ),
                ));
            }
            syn::Type::Reference(ty_ref) if ty_ref.mutability.is_some() => {
                result.mut_args.push((ident, *ty_ref.elem.clone()));
                result.injected.push(*ty_ref.elem);
            }
            syn::Type::Reference(ty_ref) => {
                result.imu_args.push((ident, *ty_ref.elem.clone()));
                result.injected.push(*ty_ref.elem);
            }
            ty => {
                result.can_args.push((ident, ty));
            }
        }
    }

    Ok(result)
}

fn collect_args(
    entry_point: EntryPoint,
    signature: &syn::Signature,
) -> Result<Vec<(Ident, syn::Type)>, Error> {
    let mut args = Vec::new();

    for (id, arg) in signature.inputs.iter().enumerate() {
        let (ident, ty) = match arg {
            syn::FnArg::Receiver(r) => {
                return Err(Error::new(
                    r.span(),
                    format!(
                        "#[{}] macro can not be used on a function with `self` as a parameter.",
                        entry_point
                    ),
                ))
            }
            syn::FnArg::Typed(syn::PatType { pat, ty, .. }) => {
                if let syn::Pat::Ident(syn::PatIdent { ident, .. }) = pat.as_ref() {
                    (ident.clone(), *ty.clone())
                } else {
                    (Ident::new(&format!("arg_{}", id), pat.span()), *ty.clone())
                }
            }
        };

        args.push((ident, ty));
    }

    Ok(args)
}
