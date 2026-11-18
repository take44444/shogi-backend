use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{ExprPath, ItemFn, Token, parse_macro_input, punctuated::Punctuated};

/// Declares the APIs this test is intended to verify.
#[proc_macro_attribute]
pub fn targets(arguments: TokenStream, item: TokenStream) -> TokenStream {
    let targets = parse_macro_input!(arguments with Punctuated::<ExprPath, Token![,]>::parse_terminated);
    let original = item.clone();
    let function = parse_macro_input!(item as ItemFn);
    if targets.is_empty() {
        return syn::Error::new_spanned(&function.sig.ident, "targets requires at least one Rust API path")
            .to_compile_error()
            .into();
    }
    let marker = format_ident!("__traceability_targets_{}", function.sig.ident);
    let cfg = function.attrs.iter().filter(|attribute| {
        attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
    });
    let references = targets.iter();
    let mut output = original;
    output.extend(TokenStream::from(quote! {
        #(#cfg)*
        #[doc(hidden)]
        #[allow(dead_code, non_snake_case)]
        fn #marker() {
            #(let _ = #references;)*
        }
    }));
    output
}
