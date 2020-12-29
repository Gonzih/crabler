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


Asynchronous web scraper engine written in rust.

Features:
* fully based on `async-std`
* derive macro based api
* struct based api
* stateful scraper (structs can hold state)
* ability to download files
* ability to schedule navigation jobs in an async manner

## Example

```rust
extern crate crabler;

use crabler::*;

#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", walk_handler)]
struct Scraper {}

impl Scraper {
    async fn response_handler(&self, response: Response) -> Result<()> {
        if response.url.ends_with(".jpg") && response.status == 200 {
            println!("Finished downloading {}", response.url);
        }
        Ok(())
    }

    async fn walk_handler(&self, response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            // attempt to download an image
            if href.ends_with(".jpg") {
                let p = Path::new("/tmp").join("image.jpg");
                let destination = p.to_string_lossy().to_string();

                if !p.exists() {
                    println!("Downloading {}", destination);
                    // schedule crawler to download file to some destination
                    // downloading will happen in the background, await here is just to wait for job queue
                    response.download_file(href, destination).await?;
                } else {
                    println!("Skipping exist file {}", destination);
                }
            } else {
              // or schedule crawler to navigate to a given url
              response.navigate(href).await?;
            };
        }

        Ok(())
    }
}

#[async_std::main]
async fn main() -> Result<()> {
    let scraper = Scraper {};

    // Run scraper starting from given url and using 20 worker threads
    scraper.run(Opts::new().with_urls(vec!["https://www.rust-lang.org/"]).with_threads(20)).await
}
```

## Sample project

[Gonzih/apod-nasa-scraper-rs](https://github.com/Gonzih/apod-nasa-scraper-rs/)
