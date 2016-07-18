#![deny(warnings)]
extern crate hyper;
extern crate mime;
extern crate env_logger;

use std::env;
use std::io;
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::clone::Clone;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Client, Request, Response, DefaultTransport as HttpStream};
use hyper::header::{Connection, ContentType, Location};
use hyper::{Url, Decoder, Encoder, Next};
use hyper::status::StatusCode;
use hyper::header::Headers;
use mime::Mime;
use mime::TopLevel::Text;
use mime::SubLevel::Html;


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
    sender: mpsc::Sender<ResponseResult>,
    result: Option<ResponseResult>
}

impl Handler {
    fn make_request(url: Url, client: &Client<Handler>, tx: mpsc::Sender<ResponseResult>) {
        let handler = Handler { url: url.clone(), sender: tx, result: None };
        client.request(url, handler).unwrap();
    }

    fn read(&self) -> Next {
        Next::read().timeout(Duration::from_secs(10))
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
                        println!("Response read error: {}", e);
                        self.return_response()
                    }
                }
            }
        } else {
            panic!();
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        println!("Some http error: {}", err);
        Next::remove()
    }
}

fn handle_redirect(response: &ResponseResult,
                   client: &Client<Handler>, tx: &mpsc::Sender<ResponseResult>) {
    println!("Handling redirect");
    match response.headers.get::<Location>() {
        Some(&Location(ref location)) => {
            if let Ok(url) = location.parse() {
                // TODO - limit redirects
                Handler::make_request(url, &client, tx.clone());
            } else {
                println!("Can not parse location url");
            }
        },
        _ => {
            println!("Can not handle redirect!");
        }
    }
}

fn crawl(client: &Client<Handler>,
         tx: mpsc::Sender<ResponseResult>, rx: mpsc::Receiver<ResponseResult>) {
    loop {
        let response = rx.recv().unwrap();
        println!("\nReceived {:?} from {} {:?}, body: {}",
                 response.status, response.url, response.headers, response.body.is_some());
        // TODO - extract links, save body
        match response.status {
            StatusCode::Ok => {
                if let Some(_body) = response.body {
                    println!("Got body, now decode and save it!");
                }
            },
            StatusCode::MovedPermanently | StatusCode::Found | StatusCode::SeeOther |
            StatusCode::TemporaryRedirect | StatusCode::PermanentRedirect => {
                handle_redirect(&response, client, &tx);
            },
            _ => {
                println!("Got unexpected status {:?}", response.status);
            }
        }
    }
}

fn main() {
    env_logger::init().unwrap();

    let filename = match env::args().nth(1) {
        Some(filename) => filename,
        None => {
            println!("Usage: client <urls file>");
            return;
        }
    };

    let (tx, rx) = mpsc::channel();
    let client = Client::new().expect("Failed to create a Client");
    let urls_file = BufReader::new(File::open(filename).unwrap());
    for line in urls_file.lines() {
        let line = line.unwrap();
        let url = format!("http://{}", line.trim());
        match url.parse() {
            Ok(url) => {
                Handler::make_request(url, &client, tx.clone());
            },
            Err(e) => {
                println!("Error parsing url '{}': {}", url, e);
            }
        }
    }

    crawl(&client, tx, rx);
    client.close();
}
