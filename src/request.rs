use std::hash::Hash;
use std::hash::Hasher;
use std::hash::SipHasher;

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

    pub fn get_fingerprint(&self) -> u64 {
        let mut hasher = SipHasher::new();
        let ref url = self.url;
        let canonical_url = format!(
            "{}://{}:{}{}?{}",
            url.scheme(),
            url.host_str().unwrap_or(""),
            url.port_or_known_default().unwrap_or(0),
            url.path(),
            url.query().unwrap_or("")); // TODO - canonicalize query
        // fragment is not included
        canonical_url.hash(&mut hasher);
        hasher.finish()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn fp(url: &str) -> u64 {
        Request::from_str(url).get_fingerprint()
    }

    #[test]
    fn test_fingerprint() {
        assert_eq!(fp("http://a.com"), fp("http://a.com"));
        assert!(fp("http://a.com") != fp("http://a.com/foo"));
        assert!(fp("http://a.com") != fp("http://b.com"));
        assert!(fp("http://a.com") != fp("https://a.com"));
        assert_eq!(fp("http://a.com/"), fp("http://a.com"));
        assert_eq!(fp("http://a.com/#foo"), fp("http://a.com"));
        assert_eq!(fp("http://a.com/b#foo"), fp("http://a.com/b"));
        assert!(fp("http://a.com/b") != fp("http://a.com/b?a=1"));
        assert_eq!(fp("http://a.com/b"), fp("http://a.com/b?"));
    }
}
