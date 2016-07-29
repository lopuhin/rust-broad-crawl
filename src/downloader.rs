use std::io;
use std::sync::mpsc;
use std::time::Duration;

use hyper;
use hyper::client::{Client, Request as HyperRequest, Response as HyperResponse,
    DefaultTransport as HttpStream};
use hyper::header::{Connection, ContentType};
use hyper::{Decoder, Encoder, Next};
use hyper::status::StatusCode;
use hyper::header::Headers;
use mime::Mime;
use mime::TopLevel::Text;
use mime::SubLevel::Html;

use request::Request;
use response::Response;


pub type ResultSender = mpsc::Sender<(Request, Option<Response>)>;

#[derive(Debug)]
pub struct Handler {
    timeout: u64,
    sender: ResultSender,
    request: Request,
    response: Option<Response>
}

pub fn make_request(request: Request,
                    timeout: u64, client: &Client<Handler>, tx: ResultSender) {
    let url = request.url.clone();
    let handler = Handler {
        request: request, timeout: timeout, sender: tx, response: None };
    client.request(url, handler).unwrap();
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

impl Handler {
    fn read(&self) -> Next {
        Next::read().timeout(Duration::from_secs(self.timeout))
    }

    fn return_response(&self) -> Next {
        self.send_result();
        Next::end()
    }

    fn send_result(&self) {
        self.sender.send((self.request.clone(), self.response.clone())).unwrap();
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
        self.response = Some(Response {
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
        if let Some(ref mut response) = self.response {
            if response.body.is_none() {
                response.body = Some(Vec::new());
            }
            if let Some(ref mut body) = response.body {
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
                        info!("Response read error for {}: {}", self.request.url, e);
                        self.return_response()
                    }
                }
            }
        } else {
            panic!();
        }
    }

    fn on_error(&mut self, err: hyper::Error) -> Next {
        info!("Http error for {}: {}", self.request.url, err);
        self.send_result();
        Next::remove()
    }
}
