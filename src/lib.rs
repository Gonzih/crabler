use async_std::sync::RwLock;
use async_std::sync::{channel, Receiver, Sender};
pub use crabquery::{Document, Element};
use std::collections::HashSet;
use std::error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs::File;
use tokio::prelude::*;

pub use async_trait::async_trait;
pub use crabweb_derive::WebScraper;

const DEFAULT_BUFFER_SIZE: usize = 10000;

#[async_trait(?Send)]
pub trait WebScraper {
    async fn dispatch_on_html(
        &mut self,
        selector: &'static str,
        response: Response,
        element: Element,
    ) -> Result<()>;
    async fn dispatch_on_response(&mut self, response: Response) -> Result<()>;
    fn all_html_selectors(&self) -> Vec<&'static str>;
}

pub type Result<T> = std::result::Result<T, Box<dyn error::Error>>;

enum Workload {
    Navigate(String),
    Download {
        url: String,
        destination: String,
    },
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

    pub async fn navigate(&mut self, url: String) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workload_tx.send(Workload::Navigate(url)).await;

        Ok(())
    }

    pub async fn download_file(&mut self, url: String, destination: String) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        self.workload_tx.send(Workload::Download{ url, destination }).await;

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
where
    T: WebScraper,
{
    visited_links: Arc<RwLock<HashSet<String>>>,
    workload_ch: Channels<Workload>,
    workoutput_ch: Channels<WorkOutput>,
    scraper: T,
    counter: Arc<AtomicUsize>,
}

impl<T> CrabWeb<T>
where
    T: WebScraper,
{
    pub fn new(scraper: T) -> Self {
        let visited_links = Arc::new(RwLock::new(HashSet::new()));
        let workload_ch = Channels::new();
        let workoutput_ch = Channels::new();
        let counter = Arc::new(AtomicUsize::new(0));

        CrabWeb {
            visited_links,
            workload_ch,
            workoutput_ch,
            scraper,
            counter,
        }
    }

    pub async fn navigate(&mut self, url: &str) -> Result<()> {
        self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(self.workload_ch.tx.send(Workload::Navigate(url.to_string())).await)
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            let output = self.workoutput_ch.rx.recv().await;
            if let Some(output) = output {
                match output {
                    WorkOutput::Markup { text, url, status } => {
                        let document = Document::from(text);

                        let response = Response::new(
                            status,
                            url.clone(),
                            self.workload_ch.tx.clone(),
                            self.counter.clone(),
                        );
                        self.scraper.dispatch_on_response(response).await?;

                        for selector in self.scraper.all_html_selectors() {
                            for el in document.select(selector) {
                                let response = Response::new(
                                    status,
                                    url.clone(),
                                    self.workload_ch.tx.clone(),
                                    self.counter.clone(),
                                );
                                self.scraper
                                    .dispatch_on_html(selector, response, el)
                                    .await?;
                                }
                        }

                    },
                    WorkOutput::Download(_) => (),
                }

                self.counter.fetch_sub(1, Ordering::SeqCst);

                if self.counter.load(Ordering::SeqCst) == 0 {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(())
    }

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

pub struct Worker {
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

    pub async fn start(&self) -> Result<()> {
        let visited_links = self.visited_links.clone();

        loop {
            let workload = self.workload_rx.recv().await;
            if let Some(workload) = workload {
                let workoutput_tx = self.workoutput_tx.clone();

                match workload {
                    Workload::Navigate(url) => {
                        let contains = visited_links.read().await.contains(&url.clone());

                        if !contains {
                            self.visited_links.write().await.insert(url.clone());

                            let response = reqwest::get(&url).await?;
                            let payload = workoutput_from_response(response).await?;
                            workoutput_tx.send(payload).await;
                        }
                    },
                    Workload::Download{ url, destination } => {
                        let contains = visited_links.read().await.contains(&url.clone());

                        if !contains {
                            // need to notify parent about work being done
                            let response = reqwest::get(&*url).await?.bytes().await?;
                            let mut dest = File::create(destination.clone()).await?;
                            dest.write_all(&response).await?;

                            let payload = WorkOutput::Download(destination);
                            workoutput_tx.send(payload).await;
                        }
                    },
                }
            } else {
                break;
            }
        }

        Ok(())
    }
}

enum WorkOutput {
    Markup {
        url: String,
        text: String,
        status: u16,
    },
    Download(String),
}

async fn workoutput_from_response(response: reqwest::Response) -> Result<WorkOutput> {
    let url = response.url().to_string();
    let status = response.status().as_u16();
    let text = response.text().await?;

    Ok(WorkOutput::Markup { status, url, text })
}
