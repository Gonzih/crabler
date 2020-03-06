# Crabler - Web crawler for Crabs

[![CI][ci-badge]][ci-url]
[![Crates.io][crates-badge]][crates-url]
[![docs.rs][docs-badge]][docs-url]
[![MIT licensed][mit-badge]][mit-url]

[ci-badge]: https://github.com/Gonzih/crabler/workflows/CI/badge.svg
[ci-url]: https://github.com/Gonzih/crabler/actions
[crates-badge]: https://img.shields.io/crates/v/crabler.svg
[crates-url]: https://crates.io/crates/crabler
[docs-badge]: https://docs.rs/crabler/badge.svg
[docs-url]: https://docs.rs/crabler
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE

Structures as asynchronous web crawlers.

Goal of this library is to help crabs with web crawling.

## Example

```rust
extern crate crabler;

use crabler::*;

#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", print_handler)]
struct Scraper {}

impl Scraper {
    async fn response_handler(&self, response: Response) -> Result<()> {
        println!("Status {}", response.status);
        Ok(())
    }

    async fn print_handler(&self, response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            println!("Found link {} on {}", href, response.url);
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let scraper = Scraper {};

    // Run scraper starting from given url and using 20 worker threads
    scraper.run(Opts::new().with_urls(vec!["https://news.ycombinator.com/"]).with_threads(20)).await
}
```
