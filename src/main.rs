#![deny(warnings)]
#[macro_use] extern crate log;
extern crate env_logger;
extern crate html5ever;
extern crate hyper;
extern crate mime;

use std::env;
use std::io;
use std::io::{BufRead, BufReader, Write};
use std::fs::File;
use std::fs::OpenOptions;
use std::clone::Clone;
use std::collections::VecDeque;
use std::str;
use std::sync::mpsc;
use std::time::Duration;

use html5ever::tokenizer::{TokenSink, Token, Tokenizer, TokenizerOpts};
use html5ever::tendril::{StrTendril};
use hyper::client::{Client, Request as HyperRequest, Response as HyperResponse,
                    DefaultTransport as HttpStream};
use hyper::header::{Connection, ContentType, Location};
use hyper::{Url, Decoder, Encoder, Next};
use hyper::status::StatusCode;
use hyper::header::Headers;
use mime::Mime;
use mime::TopLevel::Text;
use mime::SubLevel::Html;


struct CrawlerConfig {
    timeout: u64,
    urls_path: Option<String>,
    out_path: Option<String>
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        CrawlerConfig {
            timeout: 120,
            urls_path: Some("urls.csv".to_string()),
            out_path: Some("out.jl".to_string())
        }
    }
}

#[derive(Debug, Clone)]
struct Response {
    status: StatusCode,
    headers: Headers,
    body: Option<Vec<u8>>
}

fn is_html(headers: &Headers) -> bool {
    match headers.get::<ContentType>() {
        Some(content_type) => match content_type {
            &ContentType(Mime(Text, Html, _)) => true,
            _ => false
        },
        None => false
    }
}

type ResultSender = mpsc::Sender<(Url, Option<Response>)>;
type ResultReceiver = mpsc::Receiver<(Url, Option<Response>)>;

#[derive(Debug)]
struct Handler {
    url: Url,
    timeout: u64,
    sender: ResultSender,
    result: Option<Response>
}

impl Handler {
    fn make_request(url: Url, timeout: u64, client: &Client<Handler>, tx: ResultSender) {
        let handler = Handler {
            url: url.clone(), timeout: timeout, sender: tx, result: None };
        client.request(url, handler).unwrap();
    }

    fn read(&self) -> Next {
        Next::read().timeout(Duration::from_secs(self.timeout))
    }

    fn return_response(&self) -> Next {
        self.send_result();
        Next::end()
    }

    fn send_result(&self) {
        self.sender.send((self.url.clone(), self.result.clone())).unwrap();
    }
}

impl hyper::client::Handler<HttpStream> for Handler {
    fn on_request(&mut self, req: &mut HyperRequest) -> Next {
        req.headers_mut().set(Connection::close());
        // TODO - set user-agent
        self.read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        self.read()
    }

    fn on_response(&mut self, response: HyperResponse) -> Next {
        // println!("Response: {}", response.status());
        // println!("Headers:\n{}", response.headers());
        let status = response.status();
        let headers = response.headers();
        self.result = Some(Response {
            status: status.clone(),
            headers: headers.clone(),
            body: None
        });
        match status {
            &StatusCode::Ok => {
                if is_html(headers) {
                    self.read()
                } else {
                    self.return_response()
                }
            },
            _ => self.return_response()
        }
    }

    fn on_response_readable(&mut self, decoder: &mut Decoder<HttpStream>) -> Next {
        let mut read_result = None;
        if let Some(ref mut result) = self.result {
            if result.body.is_none() {
                result.body = Some(Vec::new());
            }
            if let Some(ref mut body) = result.body {
                // TODO - check that this really appends data, not overrides
                read_result = Some(io::copy(decoder, body));
            }
        }
        if let Some(read_result) = read_result {
            match read_result {
                Ok(0) => self.return_response(),
                Ok(_) => self.read(),
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock => Next::read(),
                    _ => {
                        info!("Response read error for {}: {}", self.url, e);
                        self.return_response()
                    }
                }
            }
        } else {
            panic!();
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        info!("Http error for {}: {}", self.url, err);
        self.send_result();
        Next::remove()
    }
}

fn redirect_url(response: &Response) -> Option<Url> {
    if let Some(&Location(ref location)) = response.headers.get::<Location>() {
        location.parse().ok()
    } else {
        None
    }
}


struct LinkExtractor {
    links: Vec<StrTendril>
}

impl TokenSink for LinkExtractor {
    fn process_token(&mut self, token: Token) {
        if let Token::TagToken(tag) = token {
            if tag.name.eq_str_ignore_ascii_case("a") {
                for attr in tag.attrs {
                    if attr.name.local.eq_str_ignore_ascii_case("href") {
                        self.links.push(attr.value);
                    }
                }
            }
        }
    }
}

pub fn extract_links(body: &str, base_url: &Url) -> Vec<Url> {
    let mut tokenizer = Tokenizer::new(
        LinkExtractor{links: Vec::new()}, TokenizerOpts::default());
    tokenizer.feed(StrTendril::from(body));
    tokenizer.run();
    tokenizer.end();
    let link_extractor = tokenizer.unwrap();
    link_extractor.links.iter().filter_map(|href| base_url.join(href).ok()).collect()
}

struct RequestQueue {
    deque: VecDeque<Url>,
    max_pending: u32,
    n_pending: u32
}

impl RequestQueue {
    fn new() -> Self {
        RequestQueue {
            deque: VecDeque::new(),
            max_pending: 1024,
            n_pending: 0
        }
    }

    fn push(&mut self, url: Url) {
        self.deque.push_back(url);
    }

    fn pop(&mut self) -> Option<Url> {
        if self.n_pending < self.max_pending {
            self.deque.pop_front()
        } else {
            None
        }
    }

    fn is_empty(&self) -> bool {
        self.n_pending == 0 && self.deque.is_empty()
    }

    fn incr_pending(&mut self) {
        self.n_pending += 1;
        if self.n_pending > self.max_pending {
            panic!("n_pending is greater than max_pending");
        }
    }

    fn decr_pending(&mut self) {
        if self.n_pending == 0 {
            panic!("decr_pending expected n_pending to be positive");
        }
        self.n_pending -= 1;
    }
}

fn crawl(seeds: Vec<Url>, crawler_config: &CrawlerConfig) {
    let client = Client::new().expect("Failed to create a Client");
    let (tx, rx) = mpsc::channel();

    // TODO - map
    let mut urls_file = if let Some(ref urls_path) = crawler_config.urls_path {
        Some(OpenOptions::new().create(true).append(true).open(urls_path).unwrap())
    } else { None };
    let mut out_file = if let Some(ref out_path) = crawler_config.out_path {
        Some(OpenOptions::new().create(true).append(true).open(out_path).unwrap())
    } else { None };

    let mut request_queue = RequestQueue::new();
    for url in seeds {
        request_queue.push(url);
    }

    while !request_queue.is_empty() {
        // TODO - this should be invoked not only when response arrives
        // Send new requests, while there are any
        while let Some(url) = request_queue.pop() {
            Handler::make_request(
                url, crawler_config.timeout, &client, tx.clone());
            request_queue.incr_pending();
        }
        // Block until response or error (None) arrives
        let (request_url, response) = rx.recv().unwrap();
        // We received some response or error, decrement number of pending requests
        request_queue.decr_pending();
        if let Some(ref mut urls_file) = urls_file {
            let timestamp = 0; // TODO
            let status = if let Some(ref response) = response {
                response.status.to_string()
            } else {
                "-".to_string()
            };
            // TODO - make it really csv
            write!(urls_file, "{},{},{}\n", timestamp, status, request_url).unwrap();
        }
        if let Some(ref response) = response {
            let result = handle_response(request_url, &response, &mut request_queue);
            if let Some(response_text) = result {
                if let Some(ref mut out_file) = out_file {
                    // TODO - write json
                    write!(out_file, "{}\n", response_text).unwrap();
                }
            }
        }
    }
    client.close();
}


fn handle_response(request_url: Url, response: &Response, request_queue: &mut RequestQueue)
        -> Option<String> {
    match response.status {
        StatusCode::Ok => {
            if let Some(ref body) = response.body {
                // TODO - detect encoding
                if let Ok(ref body_text) = str::from_utf8(body) {
                    for link in extract_links(&body_text, &request_url) {
                        // TODO - an option to follow only in-domain links
                        request_queue.push(link);
                    }
                    Some(body_text.to_string())
                } else {
                    info!("Dropping non-utf8 body");
                    None
                }
            } else {
                None
            }
        },
        StatusCode::MovedPermanently | StatusCode::Found | StatusCode::SeeOther |
        StatusCode::TemporaryRedirect | StatusCode::PermanentRedirect => {
            if let Some(url) = redirect_url(&response) {
                // TODO - an option to follow only in-domain links
                // TODO - limit number of redirects
                request_queue.push(url);
            } else {
                info!("Can not handle redirect for {}: no location", request_url);
            }
            None
        },
        _ => {
            info!("Got unexpected status for {}: {:?}", request_url, response.status);
            None
        }
    }
}


pub fn parse_seed(seed: &str) -> Option<Url> {
    // TODO - not mut
    let mut url = seed.to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        url = format!("http://{}", url)
    }
    url.parse().ok()
}


fn main() {
    env_logger::init().unwrap();

    let seeds_filename = match env::args().nth(1) {
        Some(seeds_filename) => seeds_filename,
        None => {
            error!("Usage: client <urls file>");
            return;
        }
    };

    let crawler_config = CrawlerConfig::default();
    let seeds_file = BufReader::new(File::open(seeds_filename).unwrap());
    let seeds: Vec<Url> = seeds_file.lines().filter_map(|line| {
        let line = line.unwrap();
        let seed = line.trim();
        if let Some(url) = parse_seed(seed) {
            Some(url)
        } else {
            error!("Error parsing seed \"{}\"", seed);
            None
        }
    }).collect();

    crawl(seeds, &crawler_config);
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_links() {
        let base_url = "http://foo.com/a/".parse().unwrap();
        let html = "<b><a href=\"../boo.txt\">a boo</a></b>\
                    <a name=\"foo\"></a>\
                    <a href=\"http://example.com/zoo\">a zoo</a>";
        let links = extract_links(&html, &base_url);
        assert_eq!(links, vec!["http://foo.com/boo.txt".parse().unwrap(),
                               "http://example.com/zoo".parse().unwrap()])
    }

    #[test]
    fn test_parse_seed() {
        assert_eq!(parse_seed("foo.com").unwrap(), "http://foo.com".parse().unwrap());
        assert_eq!(parse_seed("http://foo.com").unwrap(), "http://foo.com".parse().unwrap());
        assert_eq!(parse_seed("https://foo.com").unwrap(), "https://foo.com".parse().unwrap());
    }
}
