use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, PatIdent, parse_macro_input, spanned::Spanned};

/// Attribute macro to convert an async function into a `rest::Middleware`（静态管线版本）。
///
/// Usage:
/// ```text
/// #[rest::middleware]
/// async fn demo(
///     app: AppContext,
///     mut req: http::Request<Body>,
///     next: rest::HandlerFunc,
/// ) -> http::Response<Body> { .. }
/// ```
/// The generated function keeps the same name and returns `rest::Middleware`, taking
/// all arguments except the last two (request and next handler). Request is assumed
/// to be the penultimate parameter, next handler the last.
pub fn middleware(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let vis = &input.vis;
    let attrs = &input.attrs;
    let sig = &input.sig;
    let ident = &sig.ident;
    let generics = &sig.generics;
    let output = &sig.output;
    let asyncness = sig.asyncness;

    let mut inputs: Vec<FnArg> = sig.inputs.iter().cloned().collect();
    if inputs.len() < 2 {
        return syn::Error::new(
            sig.span(),
            "middleware fn must take at least request and next arguments",
        )
        .to_compile_error()
        .into();
    }

    let next_arg = inputs.pop().unwrap();
    let req_arg = inputs.pop().unwrap();
    let ctx_inputs = inputs;

    let (req_pat, req_ty) = match req_arg {
        FnArg::Typed(pat) => (pat.pat, pat.ty),
        FnArg::Receiver(rcv) => {
            return syn::Error::new(rcv.span(), "middleware fn cannot take self")
                .to_compile_error()
                .into();
        }
    };

    let (next_pat, next_ty) = match next_arg {
        FnArg::Typed(pat) => (pat.pat, pat.ty),
        FnArg::Receiver(rcv) => {
            return syn::Error::new(rcv.span(), "middleware fn cannot take self")
                .to_compile_error()
                .into();
        }
    };

    // Remaining arguments are captured by value and cloned per request.
    let mut ctx_idents = Vec::new();
    for arg in &ctx_inputs {
        match arg {
            FnArg::Typed(pat) => match &*pat.pat {
                Pat::Ident(PatIdent { ident, .. }) => ctx_idents.push(ident.clone()),
                _ => {
                    return syn::Error::new(
                        pat.pat.span(),
                        "only simple identifiers are supported as captured arguments",
                    )
                    .to_compile_error()
                    .into();
                }
            },
            FnArg::Receiver(rcv) => {
                return syn::Error::new(rcv.span(), "middleware fn cannot take self")
                    .to_compile_error()
                    .into();
            }
        }
    }

    let inner_ident = format_ident!("__{}_impl", ident);

    let clone_ctx_outer: Vec<_> = ctx_idents
        .iter()
        .map(|id| quote! { let #id = #id.clone(); })
        .collect();

    let clone_ctx_inner: Vec<_> = ctx_idents
        .iter()
        .map(|id| quote! { let #id = #id.clone(); })
        .collect();

    let ctx_call: Vec<_> = ctx_idents.iter().map(|id| quote! { #id }).collect();

    let block = &input.block;

    let expanded = quote! {
        #(#attrs)*
        #vis fn #ident #generics(#(#ctx_inputs),*) -> rest::Middleware {
            #(#clone_ctx_outer)*
            rest::middleware(move |__req: #req_ty, __next: #next_ty| {
                #(#clone_ctx_inner)*
                async move { #inner_ident(#(#ctx_call,)* __req, __next).await }
            })
        }

        #(#attrs)*
        #asyncness fn #inner_ident #generics(#(#ctx_inputs,)* #req_pat: #req_ty, #next_pat: #next_ty) #output {
            #block
        }
    };

    TokenStream::from(expanded)
}
