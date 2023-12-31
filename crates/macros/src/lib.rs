use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, FnArg, Ident, ItemFn, LitStr, PatType, Token,
};

struct TaskAttributes {
    source_crate: Option<String>,
    identifier: Option<String>,
}

impl Parse for TaskAttributes {
    fn parse(input: ParseStream) -> syn::parse::Result<Self> {
        let mut source_crate = None;
        let mut identifier = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_str() {
                "source_crate" => {
                    input.parse::<Token![=]>()?;
                    let lit: LitStr = input.parse()?;
                    source_crate = Some(lit.value());
                }
                "identifier" => {
                    input.parse::<Token![=]>()?;
                    let lit: LitStr = input.parse()?;
                    identifier = Some(lit.value());
                }
                _ => return Err(syn::Error::new(ident.span(), "Unknown attribute")),
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self {
            source_crate,
            identifier,
        })
    }
}

/// Procedural macro to generate a task definition based on a function.
/// Example :
/// ```rust
/// use archimedes_task_handler::TaskHandler;
///
/// #[archimedes_macros::task(
///     // This parameter is optional, it will default to "archimedes"
///     source_crate = "archimedes_task_handler"
/// )]
/// async fn my_task(ctx: String, payload: String) -> Result<(), String> {
///    println!("{} {}", ctx, payload);
///    Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn task(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as TaskAttributes);
    let input = parse_macro_input!(item as ItemFn);

    // Extract the function name
    let name = &input.sig.ident;
    let name_inner = syn::Ident::new(&format!("{}_inner", name), name.span());
    let name_struct = syn::Ident::new(&format!("{}", name), name.span());

    // Extract the function signature, parameters, and body
    let sig = &input.sig;
    let body = &input.block;

    // Extract parameter types
    let params: Vec<_> = input.sig.inputs.iter().collect();
    let payload_type = match &params[0] {
        FnArg::Typed(PatType { ty, .. }) => ty,
        _ => panic!("Expected a typed argument for payload"),
    };
    let ctx_type = match &params[1] {
        FnArg::Typed(PatType { ty, .. }) => ty,
        _ => panic!("Expected a typed argument for context"),
    };

    // Create new function signature with the new name
    let new_sig = syn::Signature {
        ident: name_inner.clone(),
        ..sig.clone()
    };

    // Extract the source crate
    let crate_source_task_handler = match args.source_crate {
        Some(crate_name) => {
            let crate_name = syn::Ident::new(&crate_name, name.span());
            quote! { #crate_name }
        }
        None => quote! { archimedes },
    };

    // Extract the identifier
    let identifier = match args.identifier {
        Some(identifier) => {
            let identifier = syn::Ident::new(&identifier, name.span());
            quote! { stringify!(#identifier) }
        }
        None => quote! { stringify!(#name_struct) },
    };

    // Generate the output tokens
    let output = quote! {
        #[allow(non_camel_case_types)]
        struct #name_struct;
        #new_sig {
            #body
        }
        impl #crate_source_task_handler::TaskDefinition<#ctx_type> for #name_struct {
            type Payload = #payload_type;
            fn get_task_runner(&self) -> impl #crate_source_task_handler::TaskHandler<#payload_type, #ctx_type> + Clone + 'static {
                #name_inner
            }
            fn identifier() -> &'static str {
                #identifier
            }
        }
    };

    output.into()
}
