use std::io::{Write};
use std::fs::{File, OpenOptions};
use std::clone::Clone;
use std::str;
use std::sync::mpsc;
use std::time::Duration;

use hyper::client::Client;
use hyper::header::{Location};
use hyper::Url;
use hyper::status::StatusCode;

use downloader::{Handler, make_request};
use link_extraction::extract_links;
use queue::RequestQueue;
use request::Request;
use response::Response;
use settings::Settings;
use stats::CrawlStats;


pub fn crawl(seeds: Vec<Url>, settings: &Settings) {
    let client = Client::<Handler>::configure()
        .max_sockets(16384)
        .connect_timeout(Duration::from_secs(120))
        .build().expect("Failed to create a Client");
    let (tx, rx) = mpsc::channel();

    // TODO - map
    let mut urls_file = if let Some(ref urls_path) = settings.urls_path {
        Some(OpenOptions::new().create(true).append(true).open(urls_path).unwrap())
    } else { None };
    let mut out_file = if let Some(ref out_path) = settings.out_path {
        Some(OpenOptions::new().create(true).append(true).open(out_path).unwrap())
    } else { None };

    let mut stats = CrawlStats::new(Duration::from_secs(10));

    let mut request_queue = RequestQueue::new(settings.concurrent_requests_per_domain);
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
        if let Some(ref mut urls_file) = urls_file {
            write_response_log(urls_file, &request, &response);
        }
        if let Some(ref response) = response {
            let result = handle_response(&request, &response, &mut request_queue);
            if let Some(response_text) = result {
                if let Some(ref mut out_file) = out_file {
                    // TODO - write json
                    write!(out_file, "{}\n", response_text).unwrap();
                }
            }
        }
        stats.maybe_report();
    }
    client.close();
}

fn handle_response(request: &Request, response: &Response, request_queue: &mut RequestQueue)
    -> Option<String> {
    match response.status {
        StatusCode::Ok => {
            if let Some(ref body) = response.body {
                // TODO - detect encoding
                if let Ok(ref body_text) = str::from_utf8(body) {
                    for link in extract_links(&body_text, &request.url) {
                        // TODO - an option to follow only in-domain links
                        request_queue.push(Request::new(link));
                    }
                    Some(body_text.to_string())
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

fn write_response_log(urls_file: &mut File, request: &Request, response: &Option<Response>) {
    let timestamp = 0; // TODO
    let status = if let &Some(ref response) = response {
        response.status.to_string()
    } else {
        "-".to_string()
    };
    // TODO - make it really csv
    write!(urls_file, "{},{},{}\n", timestamp, status, request.url).unwrap();
}
