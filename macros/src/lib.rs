use proc_macro::TokenStream;

mod rest;

#[proc_macro_attribute]
pub fn handler(attr: TokenStream, item: TokenStream) -> TokenStream {
    rest::handler::handler(attr, item)
}

#[proc_macro_attribute]
pub fn middleware(attr: TokenStream, item: TokenStream) -> TokenStream {
    rest::middleware::middleware(attr, item)
}
