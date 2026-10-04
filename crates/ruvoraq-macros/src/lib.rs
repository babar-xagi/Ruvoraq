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
    for (index, argument) in signature.inputs.iter().enumerate() {
        if let syn::FnArg::Typed(argument) = argument {
            if let syn::Type::Path(ty) = argument.ty.as_ref() {
                if ty.path.segments.last().is_some_and(|segment| {
                    segment.ident == "Json" || segment.ident == "ValidatedJson"
                }) && index + 1 != signature.inputs.len()
                {
                    return syn::Error::new_spanned(argument,
                        "Ruvoraq body extractors Json/ValidatedJson must be the last handler argument")
                        .to_compile_error().into();
                }
            }
        }
    }
    let value = path.value();
    if !value.starts_with('/')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '?' | '#' | '*'))
        || value.split('/').any(|part| part.starts_with(':'))
    {
        return syn::Error::new_spanned(
            &path,
            "Ruvoraq route paths must start with '/' (no queries, fragments, wildcards or legacy :parameters)",
        ).to_compile_error().into();
    }
    let mut parameters = std::collections::HashSet::new();
    for segment in value.split('/') {
        if segment.contains('{') || segment.contains('}') {
            let valid = segment
                .strip_prefix('{')
                .and_then(|part| part.strip_suffix('}'))
                .is_some_and(|name| {
                    let mut chars = name.chars();
                    chars
                        .next()
                        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
                        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                        && parameters.insert(name)
                });
            if !valid {
                return syn::Error::new_spanned(&path,
                    "Ruvoraq path parameters must be unique identifiers in complete {name} segments")
                    .to_compile_error().into();
            }
        }
    }

    let name = &signature.ident;
    let builder = syn::Ident::new(&method.to_ascii_lowercase(), name.span());
    let arguments: Vec<_> = signature
        .inputs
        .iter()
        .enumerate()
        .map(|(index, input)| {
            let syn::FnArg::Typed(input) = input else {
                unreachable!("receiver rejected above")
            };
            let name = quote::format_ident!("__ruvoraq_argument_{index}");
            (name, &input.ty)
        })
        .collect();
    let argument_names: Vec<_> = arguments.iter().map(|(name, _)| name).collect();
    let argument_types: Vec<_> = arguments.iter().map(|(_, ty)| ty).collect();
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
                dependencies: || {
                    use ::ruvoraq::__private::RequiredService as _;
                    let dependencies: &[Option<::ruvoraq::__private::Dependency>] = &[
                        #((&&::ruvoraq::__private::ServiceProbe::<#argument_types>::default()).required_service()),*
                    ];
                    dependencies.iter().flatten().copied().collect()
                },
                register: |app: ::ruvoraq::App| app.#builder(
                    #path,
                    |#(#argument_names: #argument_types),*| async move {
                        use ::ruvoraq::__private::Respond as _;
                        let output = ::ruvoraq::__private::HandlerOutput::new(
                            #name(#(#argument_names),*).await
                        );
                        (&&&output).respond()
                    },
                ),
            }
        }
    }
    .into()
}

/// Register an async handler for a GET path.
#[proc_macro_attribute]
pub fn get(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("GET", attribute, item)
}

/// Register an async handler for a POST path.
#[proc_macro_attribute]
pub fn post(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("POST", attribute, item)
}

/// Register an async handler for a PUT path.
#[proc_macro_attribute]
pub fn put(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("PUT", attribute, item)
}

/// Register an async handler for a PATCH path.
#[proc_macro_attribute]
pub fn patch(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("PATCH", attribute, item)
}

/// Register an async handler for a DELETE path.
#[proc_macro_attribute]
pub fn delete(attribute: TokenStream, item: TokenStream) -> TokenStream {
    route("DELETE", attribute, item)
}

/// Generate the binary entry point and load handlers from main.rs.
///
/// Invoke once at the root of settings.rs, which Cargo uses as the binary target.
/// The file must provide a settings() function returning ruvoraq::Settings.
/// Optionally use bootstrap!(configure) with a function accepting and returning App
/// to register shared services before startup.
#[proc_macro]
pub fn bootstrap(input: TokenStream) -> TokenStream {
    let configure = if input.is_empty() {
        quote! {}
    } else {
        let path = parse_macro_input!(input as syn::Path);
        quote! { let app = #path(app); }
    };

    quote! {
        #[path = "main.rs"]
        mod __ruvoraq_routes;

        fn main() -> ::std::io::Result<()> {
            ::ruvoraq::run(async {
                let app = ::ruvoraq::App::auto()?.settings(settings());
                #configure
                app.run().await
            })
        }
    }
    .into()
}
