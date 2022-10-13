use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, Error};

#[derive(Default)]
pub struct ProcessedArgs {
    pub(crate) args: Vec<Ident>,
    pub(crate) mut_args: Vec<(Ident, syn::Type)>,
    pub(crate) imu_args: Vec<(Ident, syn::Type)>,
    pub(crate) can_args: Vec<(Ident, syn::Type)>,
    injected: Vec<syn::Type>,
}

pub fn di(args: Vec<(Ident, syn::Type)>, is_async: bool) -> Result<ProcessedArgs, Error> {
    let mut result = ProcessedArgs::default();

    for (ident, ty) in args {
        result.args.push(ident.clone());

        match ty {
            syn::Type::Reference(ty_ref) if is_async => {
                return Err(Error::new(
                    ty_ref.span(),
                    "IC-Kit's dependency injection can only work on sync methods.".to_string(),
                ));
            }
            syn::Type::Reference(ty_ref) if !result.can_args.is_empty() => {
                return Err(Error::new(
                    ty_ref.span(),
                    "An IC-kit dependency injected reference could only come before canister arguments.".to_string(),
                ));
            }
            syn::Type::Reference(ty_ref) if result.injected.contains(&ty_ref.elem) => {
                return Err(Error::new(
                    ty_ref.span(),
                    "IC-Kit's dependency injection can only inject one instance of each type."
                        .to_string(),
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

pub fn collect_args(
    entry_point: &str,
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
                ));
            }
            syn::FnArg::Typed(syn::PatType { pat, ty, .. }) => {
                if let syn::Pat::Ident(syn::PatIdent { ident, .. }) = pat.as_ref() {
                    (ident.clone(), *ty.clone())
                } else {
                    (
                        Ident::new(&format!("_di_arg_{}", id), pat.span()),
                        *ty.clone(),
                    )
                }
            }
        };

        args.push((ident, ty));
    }

    Ok(args)
}

pub fn wrap(inner: TokenStream, args: ProcessedArgs) -> TokenStream {
    let mut result = inner;
    let (imu_args, imu_types): (Vec<_>, Vec<_>) = args.imu_args.into_iter().unzip();
    let (mut_args, mut_types): (Vec<_>, Vec<_>) = args.mut_args.into_iter().unzip();

    result = match imu_args.len() {
        0 => result,
        1 => {
            quote! {
                ic_kit::ic::with(|#(#imu_args: &#imu_types),*| {
                    #result
                })
            }
        }
        _ => quote! {
            ic_kit::ic::with_many(|(#(#imu_args),*) : (#(&#imu_types),*)| {
                #result
            })
        },
    };

    result = match mut_args.len() {
        0 => result,
        1 => quote! {
            ic_kit::ic::with_mut(|#(#mut_args: &mut #mut_types),*| {
                #result
            })
        },
        _ => quote! {
            ic_kit::ic::with_many_mut(|(#(#mut_args),*) : (#(&mut #mut_types),*)| {
                #result
            })
        },
    };

    result
}
