extern crate crabler;

use crabler::*;
use std::sync::{Arc, RwLock};

// Uses Arc<RwLock<...>> so results survive after run() consumes self.
#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", link_handler)]
struct TestScraper {
    responses_seen: Arc<RwLock<Vec<u16>>>,
    links_seen: Arc<RwLock<Vec<String>>>,
}

impl TestScraper {
    async fn response_handler(&mut self, response: Response) -> Result<()> {
        self.responses_seen.write().unwrap().push(response.status);
        Ok(())
    }

    async fn link_handler(&mut self, _response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            self.links_seen.write().unwrap().push(href);
        }
        Ok(())
    }
}

fn make_scraper() -> (
    TestScraper,
    Arc<RwLock<Vec<u16>>>,
    Arc<RwLock<Vec<String>>>,
) {
    let responses_seen = Arc::new(RwLock::new(vec![]));
    let links_seen = Arc::new(RwLock::new(vec![]));
    let scraper = TestScraper {
        responses_seen: responses_seen.clone(),
        links_seen: links_seen.clone(),
    };
    (scraper, responses_seen, links_seen)
}

/// mockito::Server::new() blocks briefly to create an internal tokio runtime.
/// Run it on a blocking thread to avoid stalling the async-std executor.
async fn new_mock_server() -> mockito::ServerGuard {
    async_std::task::spawn_blocking(mockito::Server::new).await
}

#[async_std::test]
async fn test_200_response() {
    let mut server = new_mock_server().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(r#"<html><body><a href="/page2">link</a></body></html>"#)
        .create();

    let (scraper, responses_seen, links_seen) = make_scraper();
    let url = server.url();

    scraper
        .run(Opts::new().with_urls(vec![url.as_str()]).with_threads(1))
        .await
        .unwrap();

    assert!(
        responses_seen.read().unwrap().contains(&200),
        "Expected status 200 in response_handler"
    );
    assert!(
        links_seen.read().unwrap().iter().any(|l| l == "/page2"),
        "Expected link /page2 from html selector"
    );
}

#[async_std::test]
async fn test_html_selector_fires_for_multiple_links() {
    let mut server = new_mock_server().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(
            r#"<html><body>
                <a href="/link1">first</a>
                <a href="/link2">second</a>
                <a href="/link3">third</a>
            </body></html>"#,
        )
        .create();

    let (scraper, _responses_seen, links_seen) = make_scraper();
    let url = server.url();

    scraper
        .run(Opts::new().with_urls(vec![url.as_str()]).with_threads(1))
        .await
        .unwrap();

    let links = links_seen.read().unwrap();
    assert!(links.iter().any(|l| l == "/link1"), "Expected /link1");
    assert!(links.iter().any(|l| l == "/link2"), "Expected /link2");
    assert!(links.iter().any(|l| l == "/link3"), "Expected /link3");
}

#[async_std::test]
async fn test_301_redirect_followed() {
    let mut server = new_mock_server().await;
    let base_url = server.url();

    // surf's Redirect middleware follows this transparently; final status is 200
    let _mock_redirect = server
        .mock("GET", "/")
        .with_status(301)
        .with_header("location", &format!("{}/final", base_url))
        .with_body("")
        .create();

    let _mock_final = server
        .mock("GET", "/final")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(r#"<html><body><a href="/found">found</a></body></html>"#)
        .create();

    let (scraper, responses_seen, links_seen) = make_scraper();

    scraper
        .run(
            Opts::new()
                .with_urls(vec![base_url.as_str()])
                .with_threads(1),
        )
        .await
        .unwrap();

    assert!(
        responses_seen.read().unwrap().contains(&200),
        "Expected status 200 after following 301 redirect"
    );
    assert!(
        links_seen.read().unwrap().iter().any(|l| l.contains("/found")),
        "Expected link from redirected page"
    );
}

#[async_std::test]
async fn test_302_redirect_followed() {
    let mut server = new_mock_server().await;
    let base_url = server.url();

    let _mock_redirect = server
        .mock("GET", "/")
        .with_status(302)
        .with_header("location", &format!("{}/final", base_url))
        .with_body("")
        .create();

    let _mock_final = server
        .mock("GET", "/final")
        .with_status(200)
        .with_header("content-type", "text/html")
        .with_body(r#"<html><body><p>Arrived at destination</p></body></html>"#)
        .create();

    let (scraper, responses_seen, _links_seen) = make_scraper();

    scraper
        .run(
            Opts::new()
                .with_urls(vec![base_url.as_str()])
                .with_threads(1),
        )
        .await
        .unwrap();

    assert!(
        responses_seen.read().unwrap().contains(&200),
        "Expected status 200 after following 302 redirect"
    );
}
