//! Goal of this library is to help crabs with web crawling.
//!
//!```ignore
//!extern crate crabler;
//!
//!use crabler::*;
//!
//!#[derive(WebScraper)]
//!#[on_response(response_handler)]
//!#[on_html("a[href]", print_handler)]
//!struct Scraper {}
//!
//!impl Scraper {
//!    async fn response_handler(&self, response: Response) -> Result<()> {
//!        debugln!("Status {}", response.status);
//!        Ok(())
//!    }
//!
//!    async fn print_handler(&self, response: Response, a: Element) -> Result<()> {
//!        if let Some(href) = a.attr("href") {
//!            debugln!("Found link {} on {}", href, response.url);
//!        }
//!
//!        Ok(())
//!    }
//!}
//!
//!#[tokio::main]
//!async fn main() -> Result<()> {
//!    let scraper = Scraper {};
//!
//!    scraper.run(Opts::new().with_urls(vec!["https://www.rust-lang.org/"])).await
//!}
//!```

mod opts;
pub use opts::*;

mod errors;
pub use errors::*;

#[macro_use]
mod debug;

use async_std::channel::{unbounded, Receiver, RecvError, Sender};
use async_std::fs::File;
use async_std::prelude::*;
use async_std::sync::RwLock;
pub use crabquery::{Document, Element};
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub use async_trait::async_trait;
pub use crabler_derive::WebScraper;

#[async_trait(?Send)]
pub trait WebScraper {
    async fn dispatch_on_html(
        &mut self,
        selector: &str,
        response: Response,
        element: Element,
    ) -> Result<()>;
    async fn dispatch_on_response(&mut self, response: Response) -> Result<()>;
    fn all_html_selectors(&self) -> Vec<&str>;
    async fn run(self, opts: Opts) -> Result<()>;
}

#[derive(Debug)]
enum Workload {
    Navigate(String),
    Download { url: String, destination: String },
}

pub struct Response {
    pub url: String,
    pub status: u16,
    workload_tx: Sender<Workload>,
    counter: Arc<AtomicUsize>,
}

impl Response {
    fn new(
        status: u16,
        url: String,
        workload_tx: Sender<Workload>,
        counter: Arc<AtomicUsize>,
    ) -> Self {
        Response {
            status,
            url,
            workload_tx,
            counter,
        }
    }

    /// Schedule scraper to visit given url,
    /// this will be executed on one of worker tasks
    pub async fn navigate(&mut self, url: String) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workload_tx.send(Workload::Navigate(url)).await?;

        Ok(())
    }

    /// Schedule scraper to download file from url into destination path
    pub async fn download_file(&mut self, url: String, destination: String) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workload_tx
            .send(Workload::Download { url, destination })
            .await?;

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
        let (tx, rx) = unbounded();

        Self { tx, rx }
    }
}

pub struct Crabler<T>
where
    T: WebScraper,
{
    visited_links: Arc<RwLock<HashSet<String>>>,
    workload_ch: Channels<Workload>,
    workoutput_ch: Channels<WorkOutput>,
    scraper: T,
    counter: Arc<AtomicUsize>,
}

impl<T> Crabler<T>
where
    T: WebScraper,
{
    /// Create new WebScraper out of given scraper struct
    pub fn new(scraper: T) -> Self {
        let visited_links = Arc::new(RwLock::new(HashSet::new()));
        let workload_ch = Channels::new();
        let workoutput_ch = Channels::new();
        let counter = Arc::new(AtomicUsize::new(0));

        Crabler {
            visited_links,
            workload_ch,
            workoutput_ch,
            scraper,
            counter,
        }
    }

    /// Schedule scraper to visit given url,
    /// this will be executed on one of worker tasks
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .workload_ch
            .tx
            .send(Workload::Navigate(url.to_string()))
            .await?)
    }

    /// Run processing loop for the given WebScraper
    pub async fn run(&mut self) -> Result<()> {
        loop {
            let output = self.workoutput_ch.rx.recv().await?;
            let response_url;
            let response_status;

            match output {
                WorkOutput::Markup { text, url, status } => {
                    debugln!("Fetched markup from: {}", url);
                    let document = Document::from(text);
                    response_url = url.clone();
                    response_status = status;

                    let selectors = self
                        .scraper
                        .all_html_selectors()
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>();

                    debugln!("Applying selectors on: {}", url);
                    for selector in selectors {
                        debugln!("Searhing for: {}", selector);
                        for el in document.select(selector.as_str()) {
                            debugln!("Generating response for: {}", selector);
                            let response = Response::new(
                                status,
                                url.clone(),
                                self.workload_ch.tx.clone(),
                                self.counter.clone(),
                            );
                            self.scraper
                                .dispatch_on_html(selector.as_str(), response, el)
                                .await?;
                        }
                    }
                }
                WorkOutput::Download(url) => {
                    debugln!("Downloaded: {}", url);
                    response_url = url;
                    response_status = 200;
                }
                WorkOutput::Noop(url) => {
                    debugln!("Noop: {}", url);
                    response_url = url;
                    response_status = 304;
                }
            }

            let response = Response::new(
                response_status,
                response_url,
                self.workload_ch.tx.clone(),
                self.counter.clone(),
            );
            self.scraper.dispatch_on_response(response).await?;

            self.counter.fetch_sub(1, Ordering::SeqCst);

            debugln!("Done processing work output, counter is at {}", self.counter.load(Ordering::SeqCst));
            if self.counter.load(Ordering::SeqCst) == 0 {
                break;
            }
        }

        Ok(())
    }

    /// Create and start new worker tasks.
    /// Worker task will automatically exit after scraper instance is freed.
    pub fn start_worker(&self) {
        let visited_links = self.visited_links.clone();
        let workload_rx = self.workload_ch.rx.clone();
        let workoutput_tx = self.workoutput_ch.tx.clone();

        let worker = Worker::new(visited_links, workload_rx, workoutput_tx);

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

struct Worker {
    visited_links: Arc<RwLock<HashSet<String>>>,
    workload_rx: Receiver<Workload>,
    workoutput_tx: Sender<WorkOutput>,
}

impl Worker {
    fn new(
        visited_links: Arc<RwLock<HashSet<String>>>,
        workload_rx: Receiver<Workload>,
        workoutput_tx: Sender<WorkOutput>,
    ) -> Self {
        Worker {
            visited_links,
            workload_rx,
            workoutput_tx,
        }
    }

    async fn start(&self) -> Result<()> {
        let visited_links = self.visited_links.clone();

        loop {
            let workload = self.workload_rx.recv().await;
            let workoutput_tx = self.workoutput_tx.clone();

            match workload {
                Err(RecvError) => continue,
                Ok(Workload::Navigate(url)) => {
                    debugln!("Navigating to {}", url);
                    let contains = visited_links.read().await.contains(&url.clone());
                    let payload;

                    if !contains {
                        self.visited_links.write().await.insert(url.clone());

                        let response = surf::get(&url).await?;
                        debugln!("Done executing get for {}", url);
                        payload = workoutput_from_response(response, url.clone()).await?;
                    } else {
                        payload = WorkOutput::Noop(url);
                    }

                    workoutput_tx.send(payload).await?;
                }
                Ok(Workload::Download { url, destination }) => {
                    let contains = visited_links.read().await.contains(&url.clone());
                    let payload;

                    if !contains {
                        // need to notify parent about work being done
                        debugln!("Trying to download {}", url);
                        let response = surf::get(&*url).await?.body_bytes().await?;
                        let mut dest = File::create(destination.clone()).await?;
                        dest.write_all(&response).await?;

                        payload = WorkOutput::Download(destination);
                    } else {
                        payload = WorkOutput::Noop(url);
                    }

                    workoutput_tx.send(payload).await?;
                }
            }
        }
    }
}

#[derive(Debug)]
enum WorkOutput {
    Markup {
        url: String,
        text: String,
        status: u16,
    },
    Download(String),
    Noop(String),
}

async fn workoutput_from_response(mut response: surf::Response, url: String) -> Result<WorkOutput> {
    let status = response.status().into();
    let mime = response.take_body().mime().clone();
    debugln!("Extracting body from {} \n\tMIME {:?}\n\tContent-Encoding: {:?}\n\tBody mime {:?}", url, response.content_type(), response.header("Content-Type"), mime);
    let text = response.body_string().await?;

    Ok(WorkOutput::Markup { status, url, text })
}
