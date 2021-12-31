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

```rust,no_run
extern crate crabler;

use std::path::Path;

use crabler::*;
use surf::Url;

#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", walk_handler)]
struct Scraper {}

impl Scraper {
    async fn response_handler(&self, response: Response) -> Result<()> {
        if response.url.ends_with(".png") && response.status == 200 {
            println!("Finished downloading {} -> {:?}", response.url, response.download_destination);
        }
        Ok(())
    }

    async fn walk_handler(&self, mut response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            // Create absolute URL
            let url = Url::parse(&href)
                .unwrap_or_else(|_| Url::parse(&response.url).unwrap().join(&href).unwrap());

            // Attempt to download an image
            if href.ends_with(".png") {
                let image_name = url.path_segments().unwrap().last().unwrap();
                let p = Path::new("/tmp").join(image_name);
                let destination = p.to_string_lossy().to_string();

                if !p.exists() {
                    println!("Downloading {}", destination);
                    // Schedule crawler to download file to some destination
                    // downloading will happen in the background, await here is just to wait for job queue
                    response.download_file(url.to_string(), destination).await?;
                } else {
                    println!("Skipping existing file {}", destination);
                }
            } else {
              // Or schedule crawler to navigate to a given url
              response.navigate(url.to_string()).await?;
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
