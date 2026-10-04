//! Route registration without application startup code in the route file.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};

fn route(method: &str, attribute: TokenStream, item: TokenStream) -> TokenStream {
    let path = parse_macro_input!(attribute as LitStr);
    let function = parse_macro_input!(item as ItemFn);
    let signature = &function.sig;

    if signature.asyncness.is_none() {
        return syn::Error::new_spanned(
            signature,
            "Ruvoraq route handlers must be async functions",
        )
        .to_compile_error()
        .into();
    }
    if !signature.generics.params.is_empty()
        || matches!(&signature.safety, syn::Safety::Unsafe(_))
        || signature.abi.is_some()
        || signature
            .inputs
            .iter()
            .any(|input| matches!(input, syn::FnArg::Receiver(_)))
    {
        return syn::Error::new_spanned(
            signature,
            "Ruvoraq route handlers must be non-generic, safe, free async functions",
        )
        .to_compile_error()
        .into();
    }
    let value = path.value();
    if !value.starts_with('/')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '?' | '#' | '{' | '}' | '*'))
        || value.split('/').any(|part| part.starts_with(':'))
    {
        return syn::Error::new_spanned(
            &path,
            "Ruvoraq route paths must be static paths starting with '/' (no parameters, queries or fragments yet)",
        )
        .to_compile_error()
        .into();
    }

    let name = &signature.ident;
    let builder = syn::Ident::new(&method.to_ascii_lowercase(), name.span());
    // A disabled function must not leave a registration referencing it.
    let conditions: Vec<_> = function
        .attrs
        .iter()
        .filter(|attribute| {
            attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr")
        })
        .collect();

    quote! {
        #function

        #(#conditions)*
        ::ruvoraq::__private::inventory::submit! {
            ::ruvoraq::__private::RouteRegistration {
                method: #method,
                path: #path,
                register: |app: ::ruvoraq::App| app.#builder(#path, #name),
            }
        }
    }
    .into()
}

/// Register an async handler for a static GET path.
#[proc_macro_attribute]
pub fn get(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("GET", attribute, item)
}

/// Register an async handler for a static POST path.
#[proc_macro_attribute]
pub fn post(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("POST", attribute, item)
}

/// Register an async handler for a static PUT path.
#[proc_macro_attribute]
pub fn put(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("PUT", attribute, item)
}

/// Register an async handler for a static PATCH path.
#[proc_macro_attribute]
pub fn patch(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("PATCH", attribute, item)
}

/// Register an async handler for a static DELETE path.
#[proc_macro_attribute]
pub fn delete(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("DELETE", attribute, item)
}

/// Generate the binary entry point and load handlers from main.rs.
///
/// Invoke once at the root of settings.rs, which Cargo uses as the binary target.
/// The file must provide a settings() function returning ruvoraq::Settings.
#[proc_macro]
pub fn bootstrap(input: TokenStream) -> TokenStream {
    if !input.is_empty() {
        return syn::Error::new(
            proc_macro::Span::call_site().into(),
            "ruvoraq::bootstrap! takes no arguments",
        )
        .to_compile_error()
        .into();
    }

    quote! {
        #[path = "main.rs"]
        mod __ruvoraq_routes;

        fn main() -> ::std::io::Result<()> {
            ::ruvoraq::run(async {
                ::ruvoraq::App::auto()?
                    .settings(settings())
                    .run()
                    .await
            })
        }
    }
    .into()
}
