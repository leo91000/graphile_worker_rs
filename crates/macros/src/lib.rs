use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn, PatType};

/// Procedural macro to generate a task definition based on a function.
/// Example :
/// ```rust
/// use archimedes_task_handler::TaskHandler;
///
/// #[archimedes_macros::task]
/// async fn my_task(ctx: String, payload: String) -> Result<(), String> {
///    println!("{} {}", ctx, payload);
///    Ok(())
/// }
/// ```
/// This will output :
/// ```rust
/// struct my_task;
/// async fn my_task_inner(ctx: String, payload: String) -> Result<(), String> {
///    println!("{} {}", ctx, payload);
///    Ok(())
/// }
/// impl archimedes_task_handler::TaskDefinition<String> for my_task {
///    type Payload = String;
///    fn get_task_runner(&self) -> impl archimedes_task_handler::TaskHandler<Self::Payload, String> {
///        my_task_inner
///    }
/// }
/// ```
#[proc_macro_attribute]
pub fn task(_attr: TokenStream, item: TokenStream) -> TokenStream {
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
    let ctx_type = match &params[0] {
        FnArg::Typed(PatType { ty, .. }) => ty,
        _ => panic!("Expected a typed argument for context"),
    };
    let payload_type = match &params[1] {
        FnArg::Typed(PatType { ty, .. }) => ty,
        _ => panic!("Expected a typed argument for payload"),
    };

    // Create new function signature with the new name
    let new_sig = syn::Signature {
        ident: name_inner.clone(),
        ..sig.clone()
    };

    // Generate the output tokens
    let output = quote! {
        #[allow(non_camel_case_types)]
        struct #name_struct;
        #new_sig {
            #body
        }
        impl archimedes_task_handler::TaskDefinition<#ctx_type> for #name_struct {
            type Payload = #payload_type;
            fn get_task_runner(&self) -> impl archimedes_task_handler::TaskHandler<#payload_type, #ctx_type> {
                #name_inner
            }
        }
    };

    output.into()
}
