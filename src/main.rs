#![deny(warnings)]
extern crate hyper;

extern crate env_logger;

use std::env;
//use std::io;
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::{Client, Request, Response, DefaultTransport as HttpStream};
use hyper::header::Connection;
use hyper::{Decoder, Encoder, Next};

#[derive(Debug)]
struct Dump(mpsc::Sender<()>);

impl Drop for Dump {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn read() -> Next {
    Next::read().timeout(Duration::from_secs(10))
}

impl hyper::client::Handler<HttpStream> for Dump {
    fn on_request(&mut self, req: &mut Request) -> Next {
        req.headers_mut().set(Connection::close());
        read()
    }

    fn on_request_writable(&mut self, _encoder: &mut Encoder<HttpStream>) -> Next {
        read()
    }

    fn on_response(&mut self, res: Response) -> Next {
        println!("Response: {}", res.status());
        println!("Headers:\n{}", res.headers());
        read()
    }

    fn on_response_readable(&mut self, _decoder: &mut Decoder<HttpStream>) -> Next {
        Next::end()
        /*
        match io::copy(decoder, &mut io::stdout()) {
            Ok(0) => Next::end(),
            Ok(_) => read(),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => Next::read(),
                _ => {
                    println!("ERROR: {}", e);
                    Next::end()
                }
            }
        }*/
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
                // FIXME - a single dump
                client.request(url, Dump(tx.clone())).unwrap();
            },
            Err(e) => {
                println!("Error parsing url '{}': {}", url, e);
            }
        }
    }

    // wait till done
    let _  = rx.recv();
    client.close();
}
