pub type Urls = Vec<String>;
// pub type Proxies = Vec<String>;
pub type Threads = usize;

#[derive(Clone, Debug)]
pub struct Opts {
    pub urls: Urls,
    // pub proxies: Proxies,
    pub threads: Threads,
    pub follow_redirects: bool,
}

impl Opts {
    pub fn new() -> Self {
        Opts {
            urls: vec![],
            // proxies: vec![],
            threads: 1,
            follow_redirects: true,
        }
    }

    pub fn with_urls(self, input: Vec<&str>) -> Self {
        let mut new = self;
        new.urls = input.iter().map(|s| s.to_string()).collect();

        new
    }

    //     pub fn with_proxies(self, input: Vec<&str>) -> Self {
    //         let mut new = self;
    //         new.proxies = input.iter().map(|s| s.to_string()).collect();

    //         new
    //     }

    pub fn with_threads(self, input: usize) -> Self {
        let mut new = self;
        new.threads = input;

        new
    }

    pub fn with_follow_redirects(self, input: bool) -> Self {
        let mut new = self;
        new.follow_redirects = input;

        new
    }
}
