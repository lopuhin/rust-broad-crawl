#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
use std::io;
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::clone::Clone;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Client, Request, Response, DefaultTransport as HttpStream};
use hyper::header::Connection;
use hyper::{Url, Decoder, Encoder, Next};
use hyper::status::StatusCode;
use hyper::header::Headers;

#[derive(Debug)]
struct Handler {
    sender: mpsc::Sender<ResponseResult>,
    result: Option<ResponseResult>
}

#[derive(Debug, Clone)]
struct ResponseResult {
    status: StatusCode,
    headers: Headers,
    body: Option<Vec<u8>>
}

impl Handler {
    fn finish(&self) -> Next {
        self.sender.send(self.result.clone().unwrap()).unwrap();
        Next::end()
    }

    fn read(&self) -> Next {
        Next::read().timeout(Duration::from_secs(10))
    }

    fn make_request(url: Url, client: &Client<Handler>, tx: mpsc::Sender<ResponseResult>) {
        let handler = Handler { sender: tx, result: None };
        client.request(url, handler).unwrap();
    }
}

impl hyper::client::Handler<HttpStream> for Handler {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.headers_mut().set(Connection::close());
        self.read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        self.read()
    }

    fn on_response(&mut self, response: Response) -> Next {
        println!("Response: {}", response.status());
        println!("Headers:\n{}", response.headers());
        let status = response.status().clone();
        self.result = Some(ResponseResult {
            status: status,
            headers: response.headers().clone(),
            body: None
        });
        match status {
            StatusCode::Ok => self.read(),
            _ => self.finish()
        }
    }

    fn on_response_readable(&mut self, decoder: &mut Decoder<HttpStream>) -> Next {
        let mut read_result = None;
        if let Some(ref mut result) = self.result {
            if result.body.is_none() {
                result.body = Some(Vec::new());
            }
            if let Some(ref mut body) = result.body {
                 read_result = Some(io::copy(decoder, body));
            }
        }
        if let Some(read_result) = read_result {
            match read_result {
                Ok(0) => self.finish(),
                Ok(_) => self.read(),
                Err(e) => match e.kind() {
                    io::ErrorKind::WouldBlock => Next::read(),
                    _ => {
                        println!("ERROR: {}", e);
                        self.finish()
                    }
                }
            }
        } else {
            panic!();
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        println!("ERROR: {}", err);
        Next::remove()
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

    loop {
        let response_result = rx.recv().unwrap();
        println!("Received {:?}", response_result);
        // TODO - redirects, extract links
    }
    // client.close();
}
