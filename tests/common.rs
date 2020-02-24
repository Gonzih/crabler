#[allow(unused_macros)]
macro_rules! scraper_setup {
    ($e:ident) => (
        use tokio::runtime::Runtime;

        async fn run_scraper_async(scraper: Scraper) {
            let mut crabler = CrabWeb::new(scraper);

            crabler
                .navigate("https://news.ycombinator.com/")
                .await
                .unwrap();
            crabler.start_worker();

            crabler.run().await.unwrap();
        }

        fn run_scraper(scraper: Scraper) {
            let f = run_scraper_async(scraper);
            let mut rt = Runtime::new().unwrap();
            rt.block_on(f);
        }
    );
}

#[allow(unused_macros)]
macro_rules! arc_rw_lock {
    ($e:expr) => {
        Arc::new(RwLock::new($e));
    };
}
