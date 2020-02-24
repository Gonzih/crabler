# Crabler - Web crawler for Crabs

[![CI][ci-badge]][ci-url]
[![Crates.io][crates-badge]][crates-url]
[![MIT licensed][mit-badge]][mit-url]

[crates-badge]: https://img.shields.io/crates/v/crabler.svg
[crates-url]: https://crates.io/crates/crabler
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: LICENSE
[ci-badge]: https://github.com/Gonzih/crabler/workflows/CI/badge.svg
[ci-url]: https://github.com/Gonzih/crabler/actions

Structures as asynchronous web crawlers.

## Example

```rust
use crabweb::*;

#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", print_handler)]
#[on_html("td.title > a.storylink[href]", follow_handler)]
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

    async fn follow_handler(&self, mut response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            response.navigate(href).await?;
        }

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let scraper = Scraper { };

    let mut crabweb = CrabWeb::new(scraper);

    // Queue navigation task
    crabweb.navigate("https://news.ycombinator.com/").await?;

    // Create bunch of http workers
    for _ in 0..20 {
        crabweb.start_worker();
    }

    // Run main scraper loop
    crabweb.run().await?;

    Ok(())
}
```
