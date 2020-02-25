#[allow(unused_macros)]
macro_rules! scraper_setup {
    ($e:ident) => {
        fn execute(scraper: Scraper, url: &str) {
            use tokio::runtime::Runtime;

            let f = scraper.run(url, 1);
            let mut rt = Runtime::new().unwrap();
            rt.block_on(f);
        }
    };
}

#[allow(unused_macros)]
macro_rules! arc_rw_lock {
    ($e:expr) => {
        Arc::new(RwLock::new($e));
    };
}
