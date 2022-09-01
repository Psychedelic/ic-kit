use crate::EntryPoint;
use lazy_static::lazy_static;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use std::collections::BTreeMap;
use std::sync::Mutex;
use syn::{DeriveInput, Error};

struct Method {
    mode: EntryPoint,
    rust_name: String,
    _arg_names: Vec<String>,
    arg_types: Vec<String>,
    rets: Vec<String>,
}

lazy_static! {
    static ref METHODS: Mutex<BTreeMap<String, Method>> = Mutex::new(Default::default());
    static ref LIFE_CYCLES: Mutex<BTreeMap<EntryPoint, Method>> = Mutex::new(Default::default());
}

pub(crate) fn declare(
    entry_point: EntryPoint,
    rust_name: Ident,
    name: String,
    can_args: Vec<Ident>,
    can_types: Vec<syn::Type>,
    rt: &syn::ReturnType,
) -> Result<(), Error> {
    let rets = match rt {
        syn::ReturnType::Default => Vec::new(),
        syn::ReturnType::Type(_, ty) => match ty.as_ref() {
            syn::Type::Tuple(tuple) => tuple
                .elems
                .iter()
                .cloned()
                .map(remove_reference_recursive)
                .collect(),
            _ => vec![remove_reference_recursive(ty.as_ref().clone())],
        },
    };

    let method = Method {
        mode: entry_point,
        rust_name: rust_name.to_string(),
        _arg_names: can_args.iter().map(|i| i.to_string()).collect(),
        arg_types: can_types
            .iter()
            .map(|t| format!("{}", t.to_token_stream()))
            .collect(),
        rets: rets
            .into_iter()
            .map(|c| format!("{}", c.to_token_stream()))
            .collect(),
    };

    if entry_point.is_lifecycle() {
        if LIFE_CYCLES
            .lock()
            .unwrap()
            .insert(entry_point, method)
            .is_some()
        {
            return Err(Error::new(
                rust_name.span(),
                format!("Canister's '{}' method already defined.", entry_point),
            ));
        }
    } else {
        if METHODS
            .lock()
            .unwrap()
            .insert(name.clone(), method)
            .is_some()
        {
            return Err(Error::new(
                rust_name.span(),
                format!("Method '{}' is already defined.", name),
            ));
        }
    };

    Ok(())
}

pub fn export_service(input: DeriveInput, save_candid_path: Option<syn::LitStr>) -> TokenStream {
    let methods = {
        let mut map = METHODS.lock().unwrap();
        std::mem::replace(&mut *map, BTreeMap::new())
    };

    let mut life_cycles = {
        let mut map = LIFE_CYCLES.lock().unwrap();
        std::mem::replace(&mut *map, BTreeMap::new())
    };

    let mut rust_methods = Vec::new();
    rust_methods.extend(
        life_cycles
            .values()
            .map(|m| Ident::new(m.rust_name.as_str(), Span::call_site())),
    );
    rust_methods.extend(
        methods
            .values()
            .map(|m| Ident::new(m.rust_name.as_str(), Span::call_site())),
    );

    let gen_tys = methods.iter().map(
        |(
            name,
            Method {
                arg_types,
                rets,
                mode,
                ..
            },
        )| {
            let args = arg_types
                .iter()
                .map(|t| generate_arg(quote! { args }, t))
                .collect::<Vec<_>>();

            let rets = rets
                .iter()
                .map(|t| generate_arg(quote! { rets }, t))
                .collect::<Vec<_>>();

            let modes = match mode {
                EntryPoint::Update => quote! { vec![] },
                EntryPoint::Query => {
                    quote! { vec![ic_kit::candid::parser::types::FuncMode::Query] }
                }
                _ => unreachable!(),
            };

            quote! {
                {
                    let mut args = Vec::new();
                    #(#args)*
                    let mut rets = Vec::new();
                    #(#rets)*
                    let func = Function { args, rets, modes: #modes };
                    service.push((#name.to_string(), Type::Func(func)));
                }
            }
        },
    );

    let service = quote! {
        use ic_kit::candid::types::{CandidType, Function, Type};
        let mut service = Vec::<(String, Type)>::new();
        let mut env = ic_kit::candid::types::internal::TypeContainer::new();
        #(#gen_tys)*
        service.sort_unstable_by_key(|(name, _)| name.clone());
        let ty = Type::Service(service);
    };

    let actor = if let Some(init) = life_cycles.remove(&EntryPoint::Init) {
        let args = init
            .arg_types
            .iter()
            .map(|t| generate_arg(quote! { init_args }, t))
            .collect::<Vec<_>>();

        quote! {
            let mut init_args = Vec::new();
            #(#args)*
            let actor = Some(Type::Class(init_args, Box::new(ty)));
        }
    } else {
        quote! { let actor = Some(ty); }
    };

    let name = input.ident;

    let save_candid = if let Some(path) = save_candid_path {
        quote! {
            #[cfg(test)]
            #[test]
            fn ic_kit_save_candid() {
                use ic_kit::KitCanister;
                use std::env;
                use std::fs;
                use std::path::PathBuf;

                let candid = #name::candid();
                let mut path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
                path.push(#path);
                let dir = path.parent().unwrap();

                fs::create_dir_all(dir).unwrap_or_else(|e| {
                    panic!(
                        "Failed to create the directory '{}': {}",
                        dir.as_os_str().to_string_lossy(),
                        e
                    )
                });

                fs::write(&path, candid).unwrap_or_else(|e| {
                    panic!(
                        "Failed to write to the file '{}': {}",
                        path.as_os_str().to_string_lossy(),
                        e
                    )
                });

                println!("Saved candid to: {}", path.as_os_str().to_string_lossy());
            }
        }
    } else {
        quote! {}
    };

    quote! {
        impl ic_kit::KitCanister for #name {
            #[cfg(not(target_family = "wasm"))]
            fn build(canister_id: ic_kit::Principal) -> ic_kit::rt::Canister {
                ic_kit::rt::Canister::new(canister_id)
                #(
                    .with_method::<#rust_methods>()
                )*
            }

            fn candid() -> String {
                #service
                #actor
                let result = ic_kit::candid::bindings::candid::compile(&env.env, &actor);
                format!("{}", result)
            }
        }

        #save_candid
    }
}

fn generate_arg(name: TokenStream, ty: &str) -> TokenStream {
    let ty = syn::parse_str::<syn::Type>(ty).unwrap();
    quote! {
        #name.push(env.add::<#ty>());
    }
}

/// Remove the references in a type and makes it an owned type, this is used to parse the return
/// type when it's using Kit's DI.
fn remove_reference_recursive(ty: syn::Type) -> syn::Type {
    match ty {
        syn::Type::Reference(r) => *r.elem,
        syn::Type::Tuple(tuple) => syn::Type::Tuple(syn::TypeTuple {
            paren_token: tuple.paren_token,
            elems: syn::punctuated::Punctuated::from_iter(
                tuple.elems.into_iter().map(remove_reference_recursive),
            ),
        }),
        syn::Type::Group(group) => syn::Type::Group(syn::TypeGroup {
            group_token: group.group_token,
            elem: Box::new(remove_reference_recursive(*group.elem)),
        }),
        syn::Type::Paren(paren) => syn::Type::Paren(syn::TypeParen {
            paren_token: paren.paren_token,
            elem: Box::new(remove_reference_recursive(*paren.elem)),
        }),
        syn::Type::Slice(slice) => syn::Type::Slice(syn::TypeSlice {
            bracket_token: slice.bracket_token,
            elem: Box::new(remove_reference_recursive(*slice.elem)),
        }),
        t => t,
    }
}
