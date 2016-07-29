use std::collections::VecDeque;

use request::Request;


pub struct RequestQueue {
    deque: VecDeque<Request>,
    max_pending: u32,
    n_pending: u32
}

impl RequestQueue {
    pub fn new() -> Self {
        RequestQueue {
            deque: VecDeque::new(),
            max_pending: 1024,
            n_pending: 0
        }
    }

    pub fn push(&mut self, request: Request) {
        self.deque.push_back(request);
    }

    pub fn pop(&mut self) -> Option<Request> {
        if self.n_pending < self.max_pending {
            self.deque.pop_front()
        } else {
            None
        }
    }

    pub fn is_empty(&self) -> bool {
        self.n_pending == 0 && self.deque.is_empty()
    }

    pub fn incr_pending(&mut self) {
        self.n_pending += 1;
        if self.n_pending > self.max_pending {
            panic!("n_pending is greater than max_pending");
        }
    }

    pub fn decr_pending(&mut self) {
        if self.n_pending == 0 {
            panic!("decr_pending expected n_pending to be positive");
        }
        self.n_pending -= 1;
    }
}
