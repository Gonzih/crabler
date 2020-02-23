pub use crabquery::{Element, Document};
use std::collections::{HashMap, HashSet};
use std::error;
use async_std::sync::RwLock;
use async_std::sync::{channel, Sender, Receiver};
use std::sync::Arc;
use std::future::Future;

const DEFAULT_BUFFER_SIZE: usize = 1000;

type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

pub struct Request {
    pub url: String,
    navigate_tx: Sender<String>,
}

impl Request {
    fn new(url: String, navigate_tx: Sender<String>) -> Self {
        Request { url, navigate_tx }
    }

    pub async fn navigate(&mut self, url: String) -> Result<()> {
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

pub struct CrabWeb<Fut, F>
    where Fut: Future<Output=Result<()>>,
          F: Fn(Request, Element) -> Fut {

    visited_links: Arc<RwLock<HashSet<String>>>,
    on_html_callbacks: HashMap<&'static str, F>,
    navigate_ch: Channels<String>,
    markup_ch: Channels<MarkupPayload>,
}

// async fn document_from_url(url: String) -> Result<Document> {
//     let markup = reqwest::get(&url)
//         .await?
//         .text()
//         .await?;

//     Ok(Document::from(markup))
// }

impl<Fut, F> CrabWeb<Fut, F>
    where Fut: Future<Output=Result<()>>,
          F: Fn(Request, Element) -> Fut {

    pub fn new() -> Self {
        let visited_links = Arc::new(RwLock::new(HashSet::new()));
        let on_html_callbacks = HashMap::new();
        let navigate_ch = Channels::new();
        let markup_ch = Channels::new();

        CrabWeb {
            visited_links,
            on_html_callbacks,
            navigate_ch,
            markup_ch,
        }
    }

    pub async fn on_html(&mut self, selector: &'static str, f: F) {
        self.on_html_callbacks.insert(selector, f);
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        Ok(self.navigate_ch.tx.send(url.to_string()).await)
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let payload = self.markup_ch.rx.recv().await;
            if let Some(payload) = payload {
                let MarkupPayload { text, url } = payload;
                let document = Document::from(text);

                for (selector, callback) in self.on_html_callbacks.iter() {
                    for el in document.select(selector) {
                        let request = Request::new(url.clone(), self.navigate_ch.tx.clone());
                        callback(request, el).await?;
                    }
                }

            } else {
                break;
            }
        }

        Ok(())
    }

    pub fn new_worker(&self) -> Worker {
        let visited_links = self.visited_links.clone();
        let navigate_rx = self.navigate_ch.rx.clone();
        let markup_tx = self.markup_ch.tx.clone();

        Worker::new(visited_links, navigate_rx, markup_tx)
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
                    let text = response.text().await?;
                    println!("Reporting results of {}", url.clone());
                    let payload = MarkupPayload::new(url, text);
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
}

impl MarkupPayload {
    fn new(url: String, text: String) -> Self {
        Self { url, text }
    }
}
