use std::hash::Hash;
use std::hash::Hasher;
use std::hash::SipHasher;

use hyper::Url;


#[derive(Debug, Clone, PartialEq, Hash)]
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
    pub fn get_fingerprint(&self) -> u64 {
        // TODO - canonicalize url
        let mut hasher = SipHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint() {
        assert_eq!(Request::from_str("http://a.com"), Request::from_str("http://a.com"));
        assert!(Request::from_str("http://a.com") != Request::from_str("http://a.com/foo"));
    }
}
