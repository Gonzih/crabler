extern crate crabler;

use crabler::*;
use std::sync::Arc;
use std::sync::RwLock;

#[macro_use]
mod common;

#[derive(WebScraper)]
#[on_response(response_handler)]
#[on_html("a[href]", print_handler)]
struct Scraper {
    visited_links: Arc<RwLock<Vec<String>>>,
    saw_links: Arc<RwLock<Vec<String>>>,
}

impl Scraper {
    async fn response_handler(&mut self, response: Response) -> Result<()> {
        self.visited_links.write().unwrap().push(response.url);
        Ok(())
    }

    async fn print_handler(&mut self, _response: Response, a: Element) -> Result<()> {
        if let Some(href) = a.attr("href") {
            self.saw_links.write().unwrap().push(href);
        }

        Ok(())
    }
}

scraper_setup!(Scraper);

#[test]
fn test_roundtrip() {
    let saw_links = arc_rw_lock!(vec![]);
    let visited_links = arc_rw_lock!(vec![]);

    let scraper = Scraper {
        visited_links: visited_links.clone(),
        saw_links: saw_links.clone(),
    };

    run_scraper(scraper);

    assert_eq!(visited_links.read().unwrap().len(), 1);
    assert!(saw_links.read().unwrap().len() > 10);
    assert_eq!(
        visited_links.read().unwrap().first().unwrap(),
        "https://news.ycombinator.com/"
    );
}
