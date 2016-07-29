pub struct Settings {
    pub timeout: u64,
    pub urls_path: Option<String>,
    pub out_path: Option<String>,
    pub concurrent_requests_per_domain: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            timeout: 120,
            urls_path: Some("urls.csv".to_string()),
            out_path: Some("out.jl".to_string()),
            concurrent_requests_per_domain: 4,
        }
    }
}
