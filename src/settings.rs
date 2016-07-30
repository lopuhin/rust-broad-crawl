pub struct Settings {
    pub concurrent_requests_per_domain: u32,
    pub out_path: Option<String>,
    pub timeout: u64,
    pub urls_path: Option<String>,
    pub user_agent: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            concurrent_requests_per_domain: 4,
            out_path: Some("out.jl".to_string()),
            timeout: 120,
            urls_path: Some("urls.csv".to_string()),
            user_agent: "Mozilla/5.0 (X11; Linux i686) AppleWebKit/537.36 \
                        (KHTML, like Gecko) Ubuntu Chromium/43.0.2357.130 \
                        Chrome/43.0.2357.130 Safari/537.36".to_owned(),
        }
    }
}
