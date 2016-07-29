use hyper::Url;


#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub url: Url
}

impl Request {
    pub fn new(url: Url) -> Self {
        Request { url: url }
    }
    pub fn from_str(url: &str) -> Self {
        Request::new(url.parse().unwrap())
    }
}
