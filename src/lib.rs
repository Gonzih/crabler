//! Goal of this library is to help crabs with web crawling.
//!
//!```rust
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
//!        println!("Status {}", response.status);
//!        Ok(())
//!    }
//!
//!    async fn print_handler(&self, response: Response, a: Element) -> Result<()> {
//!        if let Some(href) = a.attr("href") {
//!            println!("Found link {} on {}", href, response.url);
//!        }
//!
//!        Ok(())
//!    }
//!}
//!
//!#[async_std::main]
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

use async_std::channel::{unbounded, Receiver, RecvError, Sender};
use async_std::fs::File;
use async_std::prelude::*;
use async_std::sync::RwLock;
pub use crabquery::{Document, Element};
use log::{debug, error, info, warn};
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub use async_trait::async_trait;
pub use crabler_derive::WebScraper;

#[cfg(feature = "debug")]
fn enable_logging() {
    femme::with_level(femme::LevelFilter::Info);
}

#[cfg(not(feature = "debug"))]
fn enable_logging() {}

#[async_trait(?Send)]
pub trait WebScraper {
    async fn dispatch_on_page(&mut self, page: String) -> Result<()>;
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
enum WorkInput {
    Navigate(String),
    Download { url: String, destination: String },
    Exit,
}

#[derive(Debug)]
pub struct Response {
    pub url: String,
    pub status: u16,
    pub download_destination: Option<String>,
    workinput_tx: Sender<WorkInput>,
    counter: Arc<AtomicUsize>,
}

impl Response {
    fn new(
        status: u16,
        url: String,
        download_destination: Option<String>,
        workinput_tx: Sender<WorkInput>,
        counter: Arc<AtomicUsize>,
    ) -> Self {
        Response {
            status,
            url,
            download_destination,
            workinput_tx,
            counter,
        }
    }

    /// Schedule scraper to visit given url,
    /// this will be executed on one of worker tasks
    pub async fn navigate(&mut self, url: String) -> Result<()> {
        debug!("Increasing counter by 1");
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workinput_tx.send(WorkInput::Navigate(url)).await?;

        Ok(())
    }

    /// Schedule scraper to download file from url into destination path
    pub async fn download_file(&mut self, url: String, destination: String) -> Result<()> {
        debug!("Increasing counter by 1");
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workinput_tx
            .send(WorkInput::Download { url, destination })
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
    workinput_ch: Channels<WorkInput>,
    workoutput_ch: Channels<WorkOutput>,
    scraper: T,
    counter: Arc<AtomicUsize>,
    workers: Vec<async_std::task::JoinHandle<()>>,
    surf_client: surf::Client,
}

impl<T> Crabler<T>
where
    T: WebScraper,
{
    /// Create new WebScraper out of given scraper struct
    pub fn new(scraper: T, opts: &Opts) -> Self {
        let visited_links = Arc::new(RwLock::new(HashSet::new()));
        let workinput_ch = Channels::new();
        let workoutput_ch = Channels::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let workers = vec![];
        let surf_client = if opts.follow_redirects {
            surf::client().with(surf::middleware::Redirect::default())
        } else {
            surf::client()
        };

        Crabler {
            visited_links,
            workinput_ch,
            workoutput_ch,
            scraper,
            counter,
            workers,
            surf_client,
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        for _ in self.workers.iter() {
            self.workinput_ch.tx.send(WorkInput::Exit).await?;
        }

        self.workinput_ch.tx.close();
        self.workinput_ch.rx.close();
        self.workoutput_ch.tx.close();
        self.workoutput_ch.rx.close();

        Ok(())
    }

    /// Schedule scraper to visit given url,
    /// this will be executed on one of worker tasks
    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        debug!("Increasing counter by 1");
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .workinput_ch
            .tx
            .send(WorkInput::Navigate(url.to_string()))
            .await?)
    }

    /// Run processing loop for the given WebScraper
    pub async fn run(&mut self) -> Result<()> {
        enable_logging();

        let ret = self.event_loop().await;
        self.shutdown().await?;
        ret
    }

    async fn event_loop(&mut self) -> Result<()> {
        loop {
            let output = self.workoutput_ch.rx.recv().await?;
            let response_url;
            let response_status;
            let mut response_destination = None;

            match output {
                WorkOutput::Markup { text, url, status } => {
                    info!("Fetched markup from: {}", url);
                    self.scraper.dispatch_on_page(text.clone()).await?;
                    let document = Document::from(text);
                    response_url = url.clone();
                    response_status = status;

                    let selectors = self
                        .scraper
                        .all_html_selectors()
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<_>>();

                    for selector in selectors {
                        for el in document.select(selector.as_str()) {
                            let response = Response::new(
                                status,
                                url.clone(),
                                None,
                                self.workinput_ch.tx.clone(),
                                self.counter.clone(),
                            );
                            self.scraper
                                .dispatch_on_html(selector.as_str(), response, el)
                                .await?;
                        }
                    }
                }
                WorkOutput::Download { url, destination } => {
                    debug!("Downloaded: {} -> {}", url, destination);
                    response_url = url;
                    response_destination = Some(destination);
                    response_status = 200;
                }
                WorkOutput::Noop(url) => {
                    debug!("Noop: {}", url);
                    response_url = url;
                    response_status = 304;
                }
                WorkOutput::Error(url, e) => {
                    error!("Error from {}: {}", url, e);
                    response_url = url;
                    response_status = 500;
                }
                WorkOutput::Exit => {
                    error!("Received exit output");
                    response_url = "".to_string();
                    response_status = 500;
                }
            }

            let response = Response::new(
                response_status,
                response_url,
                response_destination,
                self.workinput_ch.tx.clone(),
                self.counter.clone(),
            );
            self.scraper.dispatch_on_response(response).await?;

            debug!("Decreasing counter by 1");
            self.counter.fetch_sub(1, Ordering::SeqCst);

            let cur_count = self.counter.load(Ordering::SeqCst);
            debug!("Done processing work output, counter is at {}", cur_count);
            debug!("Queue len: {}", self.workoutput_ch.rx.len());

            if cur_count == 0 {
                return Ok(());
            }
        }
    }

    /// Create and start new worker tasks.
    /// Worker task will automatically exit after scraper instance is freed.
    pub fn start_worker(&mut self) {
        let visited_links = self.visited_links.clone();
        let workinput_rx = self.workinput_ch.rx.clone();
        let workoutput_tx = self.workoutput_ch.tx.clone();
        let surf_client = self.surf_client.clone();

        let worker = Worker::new(
            visited_links,
            workinput_rx,
            workoutput_tx,
            surf_client,
        );

        let handle = async_std::task::spawn(async move {
            loop {
                info!("🐿️ Starting http worker");

                match worker.start().await {
                    Ok(()) => {
                        info!("Shutting down worker");
                        break;
                    }
                    Err(e) => warn!("❌ Restarting worker: {}", e),
                }
            }
        });

        self.workers.push(handle);
    }
}

struct Worker {
    visited_links: Arc<RwLock<HashSet<String>>>,
    workinput_rx: Receiver<WorkInput>,
    workoutput_tx: Sender<WorkOutput>,
    surf_client: surf::Client,
}

impl Worker {
    fn new(
        visited_links: Arc<RwLock<HashSet<String>>>,
        workinput_rx: Receiver<WorkInput>,
        workoutput_tx: Sender<WorkOutput>,
        surf_client: surf::Client,
    ) -> Self {
        Worker {
            visited_links,
            workinput_rx,
            workoutput_tx,
            surf_client,
        }
    }

    async fn start(&self) -> Result<()> {
        let workoutput_tx = self.workoutput_tx.clone();

        loop {
            let workinput = self.workinput_rx.recv().await;
            if let Err(RecvError) = workinput {
                continue;
            }

            let workinput = workinput?;
            let payload = self.process_message(workinput).await;

            match payload {
                Ok(WorkOutput::Exit) => return Ok(()),
                _ => workoutput_tx.send(payload?).await?,
            }
        }
    }

    async fn process_message(&self, workinput: WorkInput) -> Result<WorkOutput> {
        match workinput {
            WorkInput::Navigate(url) => {
                let workoutput = self.navigate(url.clone()).await;

                if let Err(e) = workoutput {
                    Ok(WorkOutput::Error(url, e))
                } else {
                    workoutput
                }
            }
            WorkInput::Download { url, destination } => {
                let workoutput = self.download(url.clone(), destination).await;

                if let Err(e) = workoutput {
                    Ok(WorkOutput::Error(url, e))
                } else {
                    workoutput
                }
            }
            WorkInput::Exit => Ok(WorkOutput::Exit),
        }
    }

    async fn navigate(&self, url: String) -> Result<WorkOutput> {
        let contains = self.visited_links.read().await.contains(&url.clone());

        if !contains {
            self.visited_links.write().await.insert(url.clone());
            let response = self.surf_client.get(&url).await?;

            WorkOutput::try_from_response(response, url.clone()).await
        } else {
            Ok(WorkOutput::Noop(url))
        }
    }

    async fn download(&self, url: String, destination: String) -> Result<WorkOutput> {
        let contains = self.visited_links.read().await.contains(&url.clone());

        if !contains {
            // need to notify parent about work being done
            let response = self.surf_client.get(&*url).await?.body_bytes().await?;
            let mut dest = File::create(destination.clone()).await?;
            dest.write_all(&response).await?;

            Ok(WorkOutput::Download { url, destination })
        } else {
            Ok(WorkOutput::Noop(url))
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
    Download {
        url: String,
        destination: String,
    },
    Noop(String),
    Error(String, CrablerError),
    Exit,
}

impl WorkOutput {
    async fn try_from_response(mut response: surf::Response, url: String) -> Result<Self> {
        let status = response.status().into();
        let text = response.body_string().await?;

        if text.len() == 0 {
            error!("body length is 0")
        }

        Ok(WorkOutput::Markup { status, url, text })
    }
}
