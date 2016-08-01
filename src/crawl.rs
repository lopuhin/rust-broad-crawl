use std::io::{Write};
use std::fs::{File, OpenOptions};
use std::clone::Clone;
use std::str;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use csv;
use hyper::client::Client;
use hyper::header::{Location};
use hyper::Url;
use hyper::status::StatusCode;
use rustc_serialize::json;

use downloader::{Handler, make_request};
use link_extraction::extract_links;
use queue::RequestQueue;
use request::Request;
use response::Response;
use settings::Settings;
use stats::CrawlStats;


pub fn crawl(seeds: Vec<Url>, settings: &Settings) {
    let client = Client::<Handler>::configure()
        .max_sockets(settings.concurrent_requests as usize)
        .connect_timeout(Duration::from_secs(settings.timeout))
        .build().expect("Failed to create a Client");
    let (tx, rx) = mpsc::channel();

    let mut response_log_writer = settings.urls_path.clone().map(|ref urls_path|
        ResponseLogWriter::new(urls_path));
    let mut out_file = settings.out_path.clone().map(|ref out_path|
        OpenOptions::new().create(true).append(true).open(out_path).unwrap());

    let mut stats = CrawlStats::new(Duration::from_secs(10));

    let mut request_queue = RequestQueue::new(settings);
    for url in seeds {
        request_queue.push(Request::new(url));
    }

    while !request_queue.is_empty() {
        // Send new requests, while there are any
        while let Some(url) = request_queue.pop() {
            make_request(
                url, &client, tx.clone(), settings.timeout, &settings.user_agent);
        }
        // Block until response or error (None) arrives
        let (request, response) = rx.recv().unwrap();
        // We received some response or error, decrement number of pending requests
        request_queue.decr_pending(&request);
        stats.record_response(&response);
        if let Some(ref mut response_log_writer) = response_log_writer {
            response_log_writer.write(&request, &response);
        }
        if let Some(ref response) = response {
            let result = handle_response(&request, &response, &mut request_queue);
            if let Some(result) = result {
                if let Some(ref mut out_file) = out_file {
                    write!(out_file, "{}\n", json::encode(&result).unwrap()).unwrap();
                    out_file.flush().unwrap();
                }
            }
        }
        stats.maybe_report(&request_queue);
    }
    client.close();
}

#[derive(RustcEncodable)]
struct CrawlResult {
    body: String,
    url: String,
}

fn handle_response(request: &Request, response: &Response, request_queue: &mut RequestQueue)
        -> Option<CrawlResult> {
    match response.status {
        StatusCode::Ok => {
            if let Some(ref body) = response.body {
                // TODO - detect encoding
                if let Ok(ref body_text) = str::from_utf8(body) {
                    for link in extract_links(&body_text, &request.url) {
                        // TODO - an option to follow only in-domain links
                        request_queue.push(Request::new(link));
                    }
                    Some(CrawlResult {
                        body: body_text.to_string(),
                        url: request.url.as_str().to_owned(),
                    })
                } else {
                    info!("Dropping non utf8 body for {}", request.url);
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
                request_queue.push(Request::new(url));
            } else {
                info!("Can not handle redirect for {}: no location", request.url);
            }
            None
        },
        _ => {
            info!("Got unexpected status for {}: {:?}", request.url, response.status);
            None
        }
    }
}


fn redirect_url(response: &Response) -> Option<Url> {
    if let Some(&Location(ref location)) = response.headers.get::<Location>() {
        location.parse().ok()
    } else {
        None
    }
}

struct ResponseLogWriter {
    writer: csv::Writer<File>,
}

impl ResponseLogWriter {
    fn new(path: &str) -> Self {
        let file = OpenOptions::new().create(true).append(true).open(path).unwrap();
        ResponseLogWriter {
            writer: csv::Writer::from_writer(file),
        }
    }

    fn write(&mut self, request: &Request, response: &Option<Response>) {
        let timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => {
                let dt = duration.as_secs() as f64 + 1e-9 * duration.subsec_nanos() as f64;
                format!("{:.6}", dt)
            },
            Err(_) => "-".to_owned()
        };
        let status = if let &Some(ref response) = response {
            response.status.to_string()
        } else {
            "-".to_string()
        };
        self.writer.encode((timestamp, status, request.url.as_str())).unwrap();
        self.writer.flush().unwrap();
    }
}
