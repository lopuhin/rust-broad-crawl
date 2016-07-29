use std::collections::{HashMap, VecDeque};

use url::Host;

use request::Request;


struct DomainQueue {
    deque: VecDeque<Request>,
    n_pending: u32,
}


pub struct RequestQueue {
    deques: HashMap<Option<Host>, DomainQueue>,
    n_pending: u32,
    max_pending: u32,
    max_per_domain: u32,
}

impl RequestQueue {
    pub fn new(max_per_domain: u32) -> Self {
        RequestQueue {
            deques: HashMap::new(),
            max_pending: 1024,
            max_per_domain: max_per_domain,
            n_pending: 0,
        }
    }

    pub fn push(&mut self, request: Request) {
        let key = self.get_key(&request);
        let domain_queue = self.deques.entry(key).or_insert_with(|| {
            DomainQueue { deque: VecDeque::new(), n_pending: 0 }
        });
        domain_queue.deque.push_back(request);
    }

    pub fn pop(&mut self) -> Option<Request> {
        // Find the first domain queue that is not empty and has free slots, and pop from it.
        if self.n_pending < self.max_pending {
            // FIXME - order is not random here
            for domain_queue in self.deques.values_mut() {
                if domain_queue.n_pending < self.max_per_domain {
                    self.n_pending += 1;
                    domain_queue.n_pending += 1;
                    let request = domain_queue.deque.pop_front();
                    if request.is_some() {
                        return request;
                    }
                    // TODO - remove this empty domain queue
                }
            }
        }
        return None;
    }

    pub fn is_empty(&self) -> bool {
        // FIXME - this is not correct while we do not remove the empty domain queues
        self.n_pending == 0 && self.deques.is_empty()
    }

    pub fn decr_pending(&mut self, request: &Request) {
        if self.n_pending == 0 {
            panic!("decr_pending expected self.n_pending to be positive");
        }
        self.n_pending -= 1;
        let key = self.get_key(request);
        if let Some(domain_queue) = self.deques.get_mut(&key) {
            if domain_queue.n_pending == 0 {
                panic!("decr_pending expected domain_queue.n_pending to be positive");
            }
            domain_queue.n_pending -= 1;
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
        queue.push(Request::from_str("http://domain-1.com/a"));
        assert_eq!(queue.pop().unwrap().url.as_str(), "http://domain-1.com/a");
        assert_eq!(queue.pop(), None);
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
}
