use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse2, Error, ItemFn};

pub fn gen_test_code(_: TokenStream, item: TokenStream) -> Result<TokenStream, Error> {
    let fun: ItemFn = parse2::<ItemFn>(item.clone()).map_err(|e| {
        Error::new(
            item.span(),
            format!(
                "The #[kit_test] macro must be on top of a function.\n {}",
                e
            ),
        )
    })?;

    let signature = &fun.sig;
    let visibility = &fun.vis;
    let is_async = signature.asyncness.is_some();
    let name = &signature.ident;

    if !is_async {
        return Err(Error::new(
            Span::call_site(),
            "The #[kit_test] can only be used on top of an async function.".to_string(),
        ));
    }

    Ok(quote! {
        #[test]
        #visibility fn #name() {
            #item

            let rt = ic_kit::rt::TokioRuntimeBuilder::new_current_thread()
                .build()
                .expect("ic-kit: Could not build tokio runtime.");

            rt.block_on(async {
                let replica = ic_kit::rt::replica::Replica::default();
                #name(replica).await;
            });
        }
    })
}
