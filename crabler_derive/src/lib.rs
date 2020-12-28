extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2;
use proc_macro_error::*;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(WebScraper, attributes(on_html, on_response))]
#[proc_macro_error]
/// Macro to derive WebScraper trait on to a given struct.
/// Supported options:
/// * `#[on_html("css selector", method_name)]` - will bind given css selector to a method. When page
/// is loaded this method will be invoked for all elements that match given selector.
/// * `#[on_response(method_name)]` - will bind given method to a successful page load action.
pub fn web_scraper_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = parse_macro_input!(input as DeriveInput);

    match ast.data {
        syn::Data::Struct(syn::DataStruct { .. }) => impl_web_scraper(&ast),
        _ => abort_call_site!("#[WebScraper] only supports structs"),
    }
}

fn impl_web_scraper(ast: &syn::DeriveInput) -> TokenStream {
    use syn::*;

    let name = &ast.ident;

    let mut selectors = vec![];
    let mut matches = vec![];
    let mut responses = vec![];

    for attr in &ast.attrs {
        let meta = attr.parse_meta();

        match meta {
            Ok(Meta::List(MetaList { path, nested, .. }))
                if path.segments[0].ident == "on_html" =>
            {
                let (selector, match_clause) = handle_on_html_attr(nested);
                selectors.push(selector);
                matches.push(match_clause);
            }
            Ok(Meta::List(MetaList { path, nested, .. }))
                if path.segments[0].ident == "on_response" =>
            {
                let response = handle_on_response_attr(nested);
                responses.push(response);
            }
            Err(err) => {
                abort_call_site!("Failed to parse attribute: {}", err);
            }
            _ => {
                abort_call_site!("Unsupported arguments on attribute");
            }
        }
    }

    let gen = quote! {
        #[async_trait(?Send)]
        impl WebScraper for #name {
            async fn dispatch_on_html(
                &mut self,
                selector: &str,
                request: Response,
                element: Element,
            ) -> std::result::Result<(), CrablerError> {

                match selector {
                    #( #matches, )*
                    _ => panic!("Failed to dispatch {}", selector),
                };

                Ok(())
            }

            fn all_html_selectors(&self) -> Vec<&str> {
                vec![#( #selectors ),*]
            }

            async fn dispatch_on_response(
                &mut self,
                request: Response,
            ) -> std::result::Result<(), CrablerError> {
                #( #responses; )*

                Ok(())
            }

            async fn run(
                self,
                opts: Opts,
            ) -> std::result::Result<(), CrablerError> {
                use crabler::Crabler;

                let mut crabler = Crabler::new(self);

                for url in &opts.urls {
                    crabler.navigate(url).await?;
                }

                for _ in 0..opts.threads {
                    crabler.start_worker();
                }

                crabler.run().await
            }
        }
    };

    gen.into()
}

fn handle_on_html_attr(
    nested: syn::punctuated::Punctuated<syn::NestedMeta, syn::token::Comma>,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::*;

    let l = nested.len();
    if l < 2 {
        abort_call_site!("Not enough argument provided to on_html attribute: {}", l);
    }

    let token = match &nested[0] {
        NestedMeta::Lit(Lit::Str(lit_str)) => lit_str,
        _ => abort_call_site!("Cant find on_html selector"),
    };

    let f = match &nested[1] {
        NestedMeta::Meta(Meta::Path(Path { segments, .. })) => &segments[0].ident,
        _ => abort_call_site!("Cant find on_html selector"),
    };

    let selector = quote! { #token };
    let match_clause = quote! { #token => self.#f(request, element).await? };

    (selector, match_clause)
}

fn handle_on_response_attr(
    nested: syn::punctuated::Punctuated<syn::NestedMeta, syn::token::Comma>,
) -> proc_macro2::TokenStream {
    use syn::*;

    let l = nested.len();
    if l < 1 {
        abort_call_site!(
            "Not enough argument provided to on_response attribute: {}",
            l
        );
    }

    let f = match &nested[0] {
        NestedMeta::Meta(Meta::Path(Path { segments, .. })) => &segments[0].ident,
        _ => abort_call_site!("Cant find on_response method"),
    };

    quote! { self.#f(request).await? }
}
