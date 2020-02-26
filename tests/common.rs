#[allow(unused_macros)]
macro_rules! scraper_setup {
    ($e:ident) => {
        fn execute(scraper: Scraper, opts: Opts) {
            use tokio::runtime::Runtime;

            let f = scraper.run(opts);
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
