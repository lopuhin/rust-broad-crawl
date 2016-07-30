use std::time::{Duration, Instant};

use response::Response;


pub struct CrawlStats {
    report_every: Duration,
    last_report: Instant,
    last_stats: Stats,
    all_stats: Stats,
}

struct Stats {
    start: Instant,
    n_requests: u64,
    n_responses: u64,
    n_read_responses: u64,
    // TODO - ideally we want to know the number of text responses
    // TODO - hashmap with return codes
}

impl Stats {
    fn new() -> Self {
        Stats {
            start: Instant::now(),
            n_requests: 0,
            n_responses: 0,
            n_read_responses: 0,
        }
    }

    fn record_response(&mut self, response: &Option<Response>) {
        self.n_requests += 1;
        if let &Some(ref response) = response {
            self.n_responses += 1;
            if response.body.is_some() {
                self.n_read_responses += 1;
            }
        }
    }

    fn report(&self) {
        info!("Requests:             {}", self.n_requests);
        info!("Responses:            {}", self.n_responses);
        info!("Read responses:       {}", self.n_read_responses);
        let dt = self.start.elapsed();
        let dt_s: f64 = dt.as_secs() as f64 + 1e-9 * dt.subsec_nanos() as f64;
        info!("rpm (read responses): {:.0}", self.n_read_responses as f64 / dt_s * 60.);
    }
}

impl CrawlStats {
    pub fn new(report_every: Duration) -> Self {
        CrawlStats {
            report_every: report_every,
            last_report: Instant::now(),
            last_stats: Stats::new(),
            all_stats: Stats::new(),
        }
    }

    pub fn record_response(&mut self, response: &Option<Response>) {
        self.last_stats.record_response(response);
        self.all_stats.record_response(response);
    }

    pub fn maybe_report(&mut self) {
        let elapsed = self.last_report.elapsed();
        if elapsed < self.report_every {
            return;
        }
        info!("Crawl stats (last {:?} s):", self.report_every.as_secs());
        self.last_stats.report();
        info!("Crawl stats (overall):");
        self.all_stats.report();
        self.last_stats = Stats::new();
        self.last_report = Instant::now();
    }
}
