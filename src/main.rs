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
use std::str;
use std::sync::mpsc;
use std::time::Duration;

use html5ever::tokenizer::{TokenSink, Token, Tokenizer, TokenizerOpts};
use html5ever::tendril::{StrTendril};
use hyper::client::{Client, Request, Response, DefaultTransport as HttpStream};
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
struct ResponseResult {
    url: Url,
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

#[derive(Debug)]
struct Handler {
    url: Url,
    timeout: u64,
    sender: mpsc::Sender<ResponseResult>,
    result: Option<ResponseResult>
}

impl Handler {
    fn make_request(url: Url, timeout: u64,
                    client: &Client<Handler>, tx: mpsc::Sender<ResponseResult>) {
        let handler = Handler {
            url: url.clone(), timeout: timeout, sender: tx, result: None };
        client.request(url, handler).unwrap();
    }

    fn read(&self) -> Next {
        Next::read().timeout(Duration::from_secs(self.timeout))
    }

    fn return_response(&self) -> Next {
        self.sender.send(self.result.clone().unwrap()).unwrap();
        Next::end()
    }
}

impl hyper::client::Handler<HttpStream> for Handler {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.headers_mut().set(Connection::close());
        // TODO - set user-agent
        self.read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        self.read()
    }

    fn on_response(&mut self, response: Response) -> Next {
        // println!("Response: {}", response.status());
        // println!("Headers:\n{}", response.headers());
        let status = response.status();
        let headers = response.headers();
        self.result = Some(ResponseResult {
            url: self.url.clone(),
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
                        info!("Response read error: {}", e);
                        self.return_response()
                    }
                }
            }
        } else {
            panic!();
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        info!("Some http error: {}", err);
        Next::remove()
    }
}

fn handle_redirect(response: &ResponseResult, crawler_config: &CrawlerConfig,
                   client: &Client<Handler>, tx: &mpsc::Sender<ResponseResult>) {
    debug!("Handling redirect");
    match response.headers.get::<Location>() {
        Some(&Location(ref location)) => {
            if let Ok(url) = location.parse() {
                // TODO - limit number of redirects
                // TODO - an option to follow only in-domain links
                Handler::make_request(url, crawler_config.timeout, client, tx.clone());
            } else {
                info!("Can not parse location url");
            }
        },
        _ => {
            info!("Can not handle redirect!");
        }
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


fn crawl(client: &Client<Handler>, crawler_config: &CrawlerConfig,
         tx: mpsc::Sender<ResponseResult>, rx: mpsc::Receiver<ResponseResult>) {

    // TODO - map
    let mut urls_file = if let Some(ref urls_path) = crawler_config.urls_path {
        Some(OpenOptions::new().create(true).append(true).open(urls_path).unwrap())
    } else { None };
    let mut out_file = if let Some(ref out_path) = crawler_config.out_path {
        Some(OpenOptions::new().create(true).append(true).open(out_path).unwrap())
    } else { None };

    loop {
        let response = rx.recv().unwrap();
        debug!("\nReceived {:?} from {} {:?}, body: {}",
               response.status, response.url, response.headers, response.body.is_some());
        if let Some(ref mut urls_file) = urls_file {
            let timestamp = 0; // TODO
            // TODO - make it really csv
            write!(urls_file, "{},{},{}\n", timestamp, response.status, response.url).unwrap();
        }
        // TODO - save body
        match response.status {
            StatusCode::Ok => {
                if let Some(body) = response.body {
                    debug!("Got body, now decode and save it!");
                    // TODO - detect encoding
                    if let Ok(ref body_text) = str::from_utf8(&body) {
                        if let Some(ref mut out_file) = out_file {
                            // TODO - write json
                            write!(out_file, "{}\n", body_text).unwrap();
                        }
                        for link in extract_links(&body_text, &response.url) {
                            // TODO - an option to follow only in-domain links
                            Handler::make_request(
                                link, crawler_config.timeout, client, tx.clone());
                        }
                    } else {
                        info!("Dropping non-utf8 body");
                    }
                }
            },
            StatusCode::MovedPermanently | StatusCode::Found | StatusCode::SeeOther |
            StatusCode::TemporaryRedirect | StatusCode::PermanentRedirect => {
                handle_redirect(&response, crawler_config, client, &tx);
            },
            _ => {
                info!("Got unexpected status {:?}", response.status);
            }
        }
    }
}


pub fn parse_seed(seed: &str) -> Option<Url> {
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

    let (tx, rx) = mpsc::channel();
    let client = Client::new().expect("Failed to create a Client");
    let crawler_config = CrawlerConfig::default();
    let seeds_file = BufReader::new(File::open(seeds_filename).unwrap());
    for line in seeds_file.lines() {
        let line = line.unwrap();
        let seed = line.trim();
        if let Some(url) = parse_seed(seed) {
            Handler::make_request(url, crawler_config.timeout, &client, tx.clone());
        } else {
            error!("Error parsing seed \"{}\"", seed);
        }
    }

    crawl(&client, &crawler_config, tx, rx);
    client.close();
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
