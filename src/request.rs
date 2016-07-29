use hyper::Url;


#[derive(Debug, Clone)]
pub struct Request {
    pub url: Url
}

impl Request {
    pub fn new(url: Url) -> Self {
        Request { url: url }
    }
}
