//! Generate the Rust code for Internet Computer's [entry points] [1]
//!
//! [1]: <https://internetcomputer.org/docs/current/references/ic-interface-spec/#entry-points>

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde::Deserialize;
use serde_tokenstream::from_tokenstream;
use std::fmt::Formatter;
use syn::{
    parse2, spanned::Spanned, Error, FnArg, ItemFn, Pat, PatIdent, PatType, ReturnType, Signature,
    Type,
};

#[derive(Copy, Clone)]
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
}

#[derive(Deserialize)]
struct Config {
    name: Option<String>,
    guard: Option<String>,
}

fn collect_args(entry_point: EntryPoint, signature: &Signature) -> Result<Vec<Ident>, Error> {
    let mut args = Vec::new();

    for (id, arg) in signature.inputs.iter().enumerate() {
        let ident = match arg {
            FnArg::Receiver(r) => {
                return Err(Error::new(
                    r.span(),
                    format!(
                        "#[{}] macro can not be used on a function with `self` as a parameter.",
                        entry_point
                    ),
                ))
            }
            FnArg::Typed(PatType { pat, .. }) => {
                if let Pat::Ident(PatIdent { ident, .. }) = pat.as_ref() {
                    ident.clone()
                } else {
                    Ident::new(&format!("arg_{}", id), pat.span())
                }
            }
        };

        args.push(ident)
    }

    Ok(args)
}

/// Process a rust syntax and generate the code for processing it.
pub fn gen_entry_point_code(
    entry_point: EntryPoint,
    attr: TokenStream,
    item: TokenStream,
) -> Result<TokenStream, Error> {
    let attrs = from_tokenstream::<Config>(&attr)?;
    let fun: ItemFn = parse2::<ItemFn>(item.clone()).map_err(|e| {
        Error::new(
            item.span(),
            format!("#[{0}] must be above a function. \n{1}", entry_point, e),
        )
    })?;
    let signature = &fun.sig;
    let visibility = &fun.vis;
    let generics = &signature.generics;

    if !generics.params.is_empty() {
        return Err(Error::new(
            generics.span(),
            format!(
                "#[{}] must be above a function with no generic parameters.",
                entry_point
            ),
        ));
    }

    let is_async = signature.asyncness.is_some();

    let return_length = match &signature.output {
        ReturnType::Default => 0,
        ReturnType::Type(_, ty) => match ty.as_ref() {
            Type::Tuple(tuple) => tuple.elems.len(),
            _ => 1,
        },
    };

    if entry_point.is_lifecycle() && return_length > 0 {
        return Err(Error::new(
            Span::call_site(),
            format!("#[{}] function cannot have a return value.", entry_point),
        ));
    }

    let arg_tuple: Vec<Ident> = collect_args(entry_point, signature)?;
    let name = &signature.ident;

    let outer_function_ident = Ident::new(
        &format!("canister_{}_{}_", entry_point, name),
        Span::call_site(),
    );

    let export_name = if entry_point.is_lifecycle() {
        format!("canister_{}", entry_point)
    } else {
        format!(
            "canister_{0} {1}",
            entry_point,
            attrs.name.unwrap_or_else(|| name.to_string())
        )
    };

    let function_call = if is_async {
        quote! { #name ( #(#arg_tuple),* ) .await }
    } else {
        quote! { #name ( #(#arg_tuple),* ) }
    };

    let arg_count = arg_tuple.len();

    let return_encode = if entry_point.is_lifecycle() {
        quote! {}
    } else {
        match return_length {
            0 => quote! { ic_kit::ic_call_api_v0_::reply(()) },
            1 => quote! { ic_kit::ic_call_api_v0_::reply((result,)) },
            _ => quote! { ic_kit::ic_call_api_v0_::reply(result) },
        }
    };

    // On initialization we can actually not receive any input and it's okay, only if
    // we don't have any arguments either.
    // If the data we receive is not empty, then try to unwrap it as if it's DID.
    let arg_decode = if entry_point.is_lifecycle() && arg_count == 0 {
        quote! {}
    } else {
        quote! { let ( #( #arg_tuple, )* ) = ic_kit::ic_call_api_v0_::arg_data(); }
    };

    let guard = if let Some(guard_name) = attrs.guard {
        let guard_ident = Ident::new(&guard_name, Span::call_site());

        quote! {
            let r: Result<(), String> = #guard_ident ();
            if let Err(e) = r {
                ic_kit::ic_call_api_v0_::reject(&e);
                return;
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #visibility struct #name {}

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
            ic_kit::setup();

            #guard

            ic_kit::ic::spawn(async {
                #arg_decode
                let result = #function_call;
                #return_encode
            });
        }

        #item
    })
}
