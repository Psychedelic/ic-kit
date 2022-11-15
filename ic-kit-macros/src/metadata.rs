use compile_time_run::run_command_str;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::quote;

pub fn generate_static_string<T: ToString>(key: T, val: T) -> TokenStream {
    let val = val.to_string();
    let key = Ident::new(&key.to_string(), Span::call_site());
    let val_code = Literal::byte_string(val.as_bytes());
    let val_len = val.len();
    quote! { pub static #key: [u8; #val_len] = *#val_code; }
}

pub fn generate_metadata() -> TokenStream {
    // TODO(oz): Gracefully handle errors if the project is not a git repository
    let git_commit =
        generate_static_string("GIT_COMMIT", run_command_str!("git", "rev-parse", "HEAD"));

    let git_url = generate_static_string(
        "GIT_URL",
        run_command_str!("git", "config", "--get", "remote.origin.url"),
    );

    let cdk = generate_static_string("CDK_VERSION", "0.5.0-alpha");

    let compiler = generate_static_string(
        "COMPILER",
        run_command_str!("rustc", "--version", "--verbose"),
    );

    let dfx = generate_static_string("DFX_VERSION", run_command_str!("dfx", "--version"));

    quote!(
        #[link_section = "icp:public env:git_commit"]
        #git_commit
        #[link_section = "icp:public env:git_url"]
        #git_url
        #[link_section = "icp:public env:cdk"]
        #cdk
        #[link_section = "icp:public env:compiler"]
        #compiler
        #[link_section = "icp:public env:dfx"]
        #dfx
    )
}
