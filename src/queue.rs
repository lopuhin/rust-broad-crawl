use std::collections::{HashMap, HashSet, VecDeque};

use url::Host;

use request::Request;


struct DomainQueue {
    deque: VecDeque<Request>,
    n_pending: u32,
}


pub struct RequestQueue {
    seen_requests: HashSet<u64>,
    deques: HashMap<Option<Host>, DomainQueue>,
    n_pending: u32,
    max_pending: u32,
    max_per_domain: u32,
}

impl RequestQueue {
    pub fn new(max_per_domain: u32) -> Self {
        RequestQueue {
            seen_requests: HashSet::new(),
            deques: HashMap::new(),
            max_pending: 1024,
            max_per_domain: max_per_domain,
            n_pending: 0,
        }
    }

    pub fn push(&mut self, request: Request) {
        let fingerprint = request.get_fingerprint();
        if self.seen_requests.insert(fingerprint) {
            let key = self.get_key(&request);
            let domain_queue = self.deques.entry(key).or_insert_with(|| {
                DomainQueue { deque: VecDeque::new(), n_pending: 0 }
            });
            domain_queue.deque.push_back(request);
        }
    }

    pub fn pop(&mut self) -> Option<Request> {
        // Find the first domain queue that is not empty and has free slots, and pop from it.
        if self.n_pending < self.max_pending {
            // FIXME - order is not random here, but this is not a huge problem, because empty
            // queues are removed.
            for domain_queue in self.deques.values_mut() {
                if domain_queue.n_pending < self.max_per_domain {
                    self.n_pending += 1;
                    domain_queue.n_pending += 1;
                    let request = domain_queue.deque.pop_front();
                    if request.is_some() {
                        return request;
                    }
                }
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.n_pending == 0 && self.deques.is_empty()
    }

    pub fn decr_pending(&mut self, request: &Request) {
        if self.n_pending == 0 {
            panic!("decr_pending expected self.n_pending to be positive");
        }
        self.n_pending -= 1;
        let key = self.get_key(request);
        let mut domain_queue_empty = false;
        if let Some(domain_queue) = self.deques.get_mut(&key) {
            if domain_queue.n_pending == 0 {
                panic!("decr_pending expected domain_queue.n_pending to be positive");
            }
            domain_queue.n_pending -= 1;
            if domain_queue.n_pending == 0 && domain_queue.deque.is_empty() {
                // The queue can become empty in self.pop too, but then it will have n_pending > 0,
                // so it is enough to check that here.
                domain_queue_empty = true;
            }
        }
        if domain_queue_empty {
            self.deques.remove(&key);
        }
    }

    fn get_key(&self, request: &Request) -> Option<Host> {
        if let Some(host) = request.url.host() {
            Some(host.to_owned())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use request::Request;
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut queue = RequestQueue::new(2);
        assert_eq!(queue.is_empty(), true);
        queue.push(Request::from_str("http://domain-1.com/a"));
        assert_eq!(queue.is_empty(), false);
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-1.com/a");
        assert_eq!(queue.is_empty(), false);
        queue.decr_pending(&Request::from_str("http://domain-1.com/a"));
        assert_eq!(queue.is_empty(), true);
        assert_eq!(queue.pop(), None);
        assert_eq!(queue.is_empty(), true);
    }

    #[test]
    fn test_domain_limit() {
        let mut queue = RequestQueue::new(2);
        queue.push(Request::from_str("http://domain-1.com/a"));
        queue.push(Request::from_str("http://domain-1.com/b"));
        queue.push(Request::from_str("http://domain-1.com/c"));
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-1.com/a");
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-1.com/b");
        assert_eq!(queue.pop(), None);
        queue.push(Request::from_str("http://domain-2.com/a"));
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-2.com/a");
        assert_eq!(queue.pop(), None);
        queue.decr_pending(&Request::from_str("http://domain-1.com/a"));
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-1.com/c");
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_sampling() {
        // Run with $ cargo test test_sampling -- --nocapture
        let mut queue = RequestQueue::new(3);
        queue.push(Request::from_str("http://domain-1.com/a"));
        queue.push(Request::from_str("http://domain-1.com/b"));
        queue.push(Request::from_str("http://domain-1.com/c"));
        queue.push(Request::from_str("http://domain-2.com/a"));
        queue.push(Request::from_str("http://domain-2.com/b"));
        queue.push(Request::from_str("http://domain-2.com/c"));
        while let Some(request) = queue.pop() {
            println!("{:?}", request);
        }
    }

    #[test]
    fn test_duplicates() {
        // Run with $ cargo test test_sampling -- --nocapture
        let mut queue = RequestQueue::new(3);
        queue.push(Request::from_str("http://domain-1.com/a"));
        queue.push(Request::from_str("http://domain-1.com/a"));
        assert!(queue.pop().is_some());
        assert_eq!(queue.pop(), None);
    }
}
