extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2;
use proc_macro_error::*;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(WebScraper, attributes(on_html))]
#[proc_macro_error]
pub fn web_scraper_derive(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = parse_macro_input!(input as DeriveInput);

    match ast.data {
        syn::Data::Struct(syn::DataStruct { .. }) => impl_web_scraper(&ast),
        _ => abort_call_site!("#[WebScraper] only supports structs"),
    }
}

struct OnHtmlMeta {
    selector: String,
    handler: String,
}

fn impl_web_scraper(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;
    // eprintln!("{}", name);
    // eprintln!("{:#?}", &ast);

    let mut selectors = vec![];
    let mut matches = vec![];

    for attr in &ast.attrs {
        let meta = attr.parse_meta();

        match meta {
            Ok(attr) => {
                let (selector, match_clause) = handle_on_html_attr(attr);
                selectors.push(selector);
                matches.push(match_clause);
            }
            Err(err) => {
                abort_call_site!("Error parsing #[on_html] attribute: {}\ncorrect format is #[on_html(\"div > a\", handler_method)]", err);
            }
        }
    }

    let gen = quote! {
        #[async_trait(?Send)]
        impl WebScraper for #name {
            async fn dispatch_on_html(&self, selector: &'static str, request: Request, element: Element) -> std::result::Result<(), Box<dyn std::error::Error>> {
                println!("Dispatch {}", stringify!(#name));

                match selector {
                    #( #matches, )*
                    _ => panic!("Failed to dispatch {}", selector),
                };

                Ok(())
            }

            fn all_html_selectors(&self) -> Vec<&'static str> {
                vec![#( #selectors ),*]
            }
        }
    };

    gen.into()
}

fn handle_on_html_attr(input: syn::Meta) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    use syn::*;

    match input {
        Meta::List(MetaList { path, nested, .. }) if path.segments[0].ident == "on_html" => {
            let l = nested.len();
            if l < 2 {
                abort_call_site!("Not enough argument provided to on_html attribute: {}", l);
            }

            eprintln!("nested has {} el: {:#?}", nested.len(), nested);

            let token = match &nested[0] {
                NestedMeta::Lit(Lit::Str(lit_str)) => lit_str,
                _ => abort_call_site!("Cant find on_html selector"),
            };

            let f = match &nested[1] {
                NestedMeta::Meta(Meta::Path(Path { segments, .. })) => &segments[0].ident,
                _ => abort_call_site!("Cant find on_html selector"),
            };

            eprintln!("token {:#?}", token);
            eprintln!("f {:#?}", f);
            let selector = quote! { #token };
            let match_clause = quote! { #token => self.#f(request, element).await? };

            (selector, match_clause)

            // quote! {}
        }
        _ => (quote! {}, quote! {}),
    }
}
