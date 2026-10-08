//! Route registration without application startup code in the route file.

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, LitStr, parse_macro_input};
struct RouteArgs {
    path: LitStr,
    status: Option<syn::LitInt>,
}
impl syn::parse::Parse for RouteArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let path = input.parse()?;
        let status = if input.is_empty() {
            None
        } else {
            input.parse::<syn::Token![,]>()?;
            let key: syn::Ident = input.parse()?;
            if key != "status" {
                return Err(syn::Error::new_spanned(key, "expected status = 201"));
            }
            input.parse::<syn::Token![=]>()?;
            let status: syn::LitInt = input.parse()?;
            let value: u16 = status.base10_parse()?;
            if !(200..300).contains(&value) {
                return Err(syn::Error::new_spanned(
                    status,
                    "success status must be 200..299",
                ));
            }
            Some(status)
        };
        Ok(Self { path, status })
    }
}
fn inner_type(ty: &syn::Type, names: &[&str]) -> Option<syn::Type> {
    if let syn::Type::Path(path) = ty {
        let last = path.path.segments.last()?;
        if names.iter().any(|name| last.ident == *name) {
            if let syn::PathArguments::AngleBracketed(args) = &last.arguments {
                return args.args.iter().find_map(|arg| {
                    if let syn::GenericArgument::Type(ty) = arg {
                        Some(ty.clone())
                    } else {
                        None
                    }
                });
            }
        }
    }
    None
}
fn type_name(ty: &syn::Type) -> String {
    if let syn::Type::Path(path) = ty {
        path.path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default()
    } else {
        String::new()
    }
}
fn schema_expr(ty: &syn::Type, output: bool) -> proc_macro2::TokenStream {
    quote! { (&&::ruvoraq::__private::SchemaProbe::<#ty>::default()).describe_schema(#output) }
}

fn route(method: &str, attribute: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attribute as RouteArgs);
    let path = args.path;
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
    let mut parameter_docs = Vec::new();
    let mut body_doc = quote! { None };
    for ty in &argument_types {
        if let Some(inner) = inner_type(ty, &["Json", "ValidatedJson"]) {
            let schema = schema_expr(&inner, false);
            body_doc = quote! { Some(#schema) };
        } else if let Some(inner) = inner_type(ty, &["Path", "Query"]) {
            let kind = if type_name(ty) == "Path" {
                "path"
            } else {
                "query"
            };
            let schema = schema_expr(&inner, false);
            parameter_docs.push(quote! { parameters.extend(::ruvoraq::__private::parameters(#kind, #schema, #path)); });
        }
    }
    // Every capture must be present even for custom or aliased extractors.
    let summary = function
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| {
            if let syn::Meta::NameValue(meta) = &a.meta {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &meta.value
                {
                    return Some(s.value().trim().to_owned());
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join(" ");
    let summary = if summary.is_empty() {
        name.to_string()
    } else {
        summary
    };
    let output_type = match &signature.output {
        syn::ReturnType::Type(_, ty) => *ty.clone(),
        _ => syn::parse_quote!(()),
    };
    let output_type = inner_type(&output_type, &["Result"]).unwrap_or(output_type);
    let opaque = matches!(output_type, syn::Type::ImplTrait(_));
    let dynamic_tuple = matches!(&output_type, syn::Type::Tuple(t) if t.elems.iter().any(|ty|type_name(ty)=="StatusCode"));
    let reply = type_name(&output_type) == "Reply";
    let json_output = reply
        || type_name(&output_type) == "Json"
        || matches!(&output_type,syn::Type::Tuple(t) if t.elems.last().is_some_and(|ty|type_name(ty)=="Json"));
    let output_type = inner_type(&output_type, &["Reply", "Json"]).unwrap_or(output_type);
    let output_type = if let syn::Type::Tuple(tuple) = &output_type {
        tuple
            .elems
            .last()
            .and_then(|ty| inner_type(ty, &["Json"]))
            .unwrap_or(output_type.clone())
    } else {
        output_type
    };
    let text = matches!(&output_type,syn::Type::Reference(r) if type_name(&r.elem)=="str")
        || type_name(&output_type) == "String";
    let binary = inner_type(&output_type, &["Vec"]).is_some_and(|ty| type_name(&ty) == "u8")
        || matches!(&output_type,syn::Type::Reference(r) if matches!(&*r.elem,syn::Type::Slice(s) if type_name(&s.elem)=="u8"));
    let media = if json_output {
        "application/json"
    } else if opaque
        || matches!(
            type_name(&output_type).as_str(),
            "Response" | "StatusCode" | "HeaderMap" | "Error"
        )
        || matches!(&output_type,syn::Type::Tuple(t) if t.elems.is_empty())
    {
        ""
    } else if text {
        "text/plain"
    } else if binary {
        "application/octet-stream"
    } else {
        "application/json"
    };
    let output_schema = if binary && !json_output {
        quote! { ::ruvoraq::json!({"type":"string","format":"binary"}) }
    } else if opaque {
        quote! { ::ruvoraq::json!({}) }
    } else {
        schema_expr(&output_type, true)
    };
    let status = args
        .status
        .map(|s| s.base10_digits().to_owned())
        .unwrap_or_else(|| {
            if reply || dynamic_tuple || media.is_empty() {
                "default".into()
            } else {
                "200".into()
            }
        });
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
                document: || {
                    use ::ruvoraq::__private::DescribeSchema as _;
                    let mut parameters = Vec::new();
                    #(#parameter_docs)*
                    for segment in #path.split('/') {
                        if let Some(name) = segment.strip_prefix('{').and_then(|s|s.strip_suffix('}')) {
                            if !parameters.iter().any(|p: &::ruvoraq::Value|p["in"]=="path" && p["name"]==name) {
                                parameters.extend(::ruvoraq::__private::parameters("path", ::ruvoraq::json!({}), &format!("/{{{name}}}")));
                            }
                        }
                    }
                    ::ruvoraq::__private::operation(
                        concat!(module_path!(), "::", stringify!(#name), "_", #method),
                        #summary, module_path!(), parameters, #body_doc, #output_schema, (#status, #media)
                    )
                },
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
/// to register shared services before startup. Hooks may also return io::Result<App>.
/// Use bootstrap!(async configure) for an async App or io::Result<App> hook.
/// Loads optional local .env and typed environment overrides before the hook.
struct BootstrapArgs {
    asynchronous: bool,
    path: syn::Path,
}
impl syn::parse::Parse for BootstrapArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let asynchronous = if input.peek(syn::Token![async]) {
            input.parse::<syn::Token![async]>()?;
            true
        } else {
            false
        };
        Ok(Self {
            asynchronous,
            path: input.parse()?,
        })
    }
}

#[proc_macro]
pub fn bootstrap(input: TokenStream) -> TokenStream {
    let configure = if input.is_empty() {
        quote! {}
    } else {
        let args = parse_macro_input!(input as BootstrapArgs);
        let path = args.path;
        let call = if args.asynchronous {
            quote! {#path(app).await}
        } else {
            quote! {#path(app)}
        };
        quote! { use ::ruvoraq::__private::ConfiguredApp as _; let app = (#call).configured()?; }
    };

    quote! {
        #[path = "main.rs"]
        mod __ruvoraq_routes;

        fn main() -> ::std::io::Result<()> {
            ::ruvoraq::run(async {
                let env = ::ruvoraq::Env::load()?;
                let app = ::ruvoraq::App::auto()?.settings(settings()).environment(env)?;
                #configure
                if app.logging_enabled() { ::ruvoraq::init_logging(); }
                app.run().await
            })
        }
    }
    .into()
}

/// Add JSON Schema metadata to a Serde model without another application dependency.
#[proc_macro_attribute]
pub fn schema(attribute: TokenStream, item: TokenStream) -> TokenStream {
    if !attribute.is_empty() {
        return syn::Error::new(
            proc_macro::Span::call_site().into(),
            "#[schema] takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let item = parse_macro_input!(item as syn::DeriveInput);
    quote! {
        #[derive(::ruvoraq::schemars::JsonSchema)]
        #[schemars(crate = "::ruvoraq::schemars")]
        #item
    }
    .into()
}
