pub use crabquery::{Element, Document};
use std::collections::{HashSet};
use std::error;
use async_std::sync::RwLock;
use async_std::sync::{channel, Sender, Receiver};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

pub use async_trait::async_trait;

const DEFAULT_BUFFER_SIZE: usize = 10000;

#[async_trait(?Send)]
pub trait WebScraper {
    async fn dispatch_on_html(&mut self, selector: &'static str, response: Response, element: Element) -> Result<()>;
    async fn dispatch_on_response(&mut self, response: Response) -> Result<()>;
    fn all_html_selectors(&self) -> Vec<&'static str>;
}

pub type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

pub struct Response {
    pub url: String,
    pub status: u16,
    navigate_tx: Sender<String>,
    counter: Arc<AtomicUsize>,
}

impl Response {
    fn new(status: u16, url: String, navigate_tx: Sender<String>, counter: Arc<AtomicUsize>) -> Self {
        Response { status, url, navigate_tx, counter }
    }

    pub async fn navigate(&mut self, url: String) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.navigate_tx.send(url).await;

        Ok(())
    }
}

#[derive(Clone)]
struct Channels<T> {
    tx: Sender<T>,
    rx: Receiver<T>,
}

impl<T> Channels<T> {
    fn new() -> Self {
        let (tx, rx) = channel(DEFAULT_BUFFER_SIZE);

        Self { tx, rx }
    }
}

pub struct CrabWeb<T>
    where T: WebScraper {
    visited_links: Arc<RwLock<HashSet<String>>>,
    navigate_ch: Channels<String>,
    markup_ch: Channels<MarkupPayload>,
    scraper: T,
    counter: Arc<AtomicUsize>,
}

impl<T> CrabWeb<T>
    where T: WebScraper {

    pub fn new(scraper: T) -> Self {
        let visited_links = Arc::new(RwLock::new(HashSet::new()));
        let navigate_ch = Channels::new();
        let markup_ch = Channels::new();
        let counter = Arc::new(AtomicUsize::new(0));

        CrabWeb {
            visited_links,
            navigate_ch,
            markup_ch,
            scraper,
            counter,
        }
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(self.navigate_ch.tx.send(url.to_string()).await)
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let payload = self.markup_ch.rx.recv().await;
            if let Some(payload) = payload {
                let MarkupPayload { text, url, status } = payload;
                let document = Document::from(text);

                let response = Response::new(status, url.clone(), self.navigate_ch.tx.clone(), self.counter.clone());
                self.scraper.dispatch_on_response(response).await?;

                for selector in self.scraper.all_html_selectors() {
                    for el in document.select(selector) {
                        let response = Response::new(status, url.clone(), self.navigate_ch.tx.clone(), self.counter.clone());
                        self.scraper.dispatch_on_html(selector, response, el).await?;
                    }
                }

                self.counter.fetch_sub(1, Ordering::SeqCst);

                if self.counter.load(Ordering::SeqCst) == 0 {
                    break
                }
            } else {
                break;
            }
        }

        Ok(())
    }

    pub fn start_worker(&self) {
        let visited_links = self.visited_links.clone();
        let navigate_rx = self.navigate_ch.rx.clone();
        let markup_tx = self.markup_ch.tx.clone();

        let worker = Worker::new(visited_links, navigate_rx, markup_tx);

        tokio::spawn(async move {
            loop {
                println!("🐿️ Starting http worker");

                match worker.start().await {
                    Ok(()) => break,
                    Err(e) => println!("❌ Restarting worker: {}", e),
                }
            }
        });
    }
}

pub struct Worker {
    visited_links: Arc<RwLock<HashSet<String>>>,
    navigate_rx: Receiver<String>,
    markup_tx: Sender<MarkupPayload>,
}

impl Worker {
    fn new(visited_links: Arc<RwLock<HashSet<String>>>, navigate_rx: Receiver<String>, markup_tx: Sender<MarkupPayload>) -> Self {
        Worker { visited_links, navigate_rx, markup_tx }
    }

    pub async fn start(&self) -> Result<()> {
        let visited_links = self.visited_links.clone();

        loop {
            let url = self.navigate_rx.recv().await;
            if let Some(url) = url {
                let contains = visited_links.read().await.contains(&url.clone());
                let markup_tx = self.markup_tx.clone();

                if !contains {
                    self.visited_links.write().await.insert(url.clone());

                    let response = reqwest::get(&url).await?;
                    let url = response.url().to_string();
                    let status = response.status().as_u16();
                    let text = response.text().await?;
                    let payload = MarkupPayload::new(status, url, text);
                    markup_tx.send(payload).await;
                }
            } else {
                break;
            }
        }

        Ok(())
    }
}

struct MarkupPayload {
    url: String,
    text: String,
    status: u16,
}

impl MarkupPayload {
    fn new(status: u16, url: String, text: String) -> Self {
        Self { status, url, text }
    }
}
