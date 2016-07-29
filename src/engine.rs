use std::io::{Write};
use std::fs::OpenOptions;
use std::clone::Clone;
use std::str;
use std::sync::mpsc;

use hyper::client::Client;
use hyper::header::{Location};
use hyper::Url;
use hyper::status::StatusCode;

use request::Request;
use response::Response;
use settings::Settings;
use downloader::make_request;
use queue::RequestQueue;
use link_extraction::extract_links;


pub fn crawl(seeds: Vec<Url>, settings: &Settings) {
    // TODO - check max sockets and default timeout
    let client = Client::new().expect("Failed to create a Client");
    let (tx, rx) = mpsc::channel();

    // TODO - map
    let mut urls_file = if let Some(ref urls_path) = settings.urls_path {
        Some(OpenOptions::new().create(true).append(true).open(urls_path).unwrap())
    } else { None };
    let mut out_file = if let Some(ref out_path) = settings.out_path {
        Some(OpenOptions::new().create(true).append(true).open(out_path).unwrap())
    } else { None };

    let mut request_queue = RequestQueue::new(settings.concurrent_requests_per_domain);
    for url in seeds {
        request_queue.push(Request::new(url));
    }

    while !request_queue.is_empty() {
        // TODO - this should be invoked not only when response arrives
        // Send new requests, while there are any
        while let Some(url) = request_queue.pop() {
            make_request(
                url, settings.timeout, &client, tx.clone());
        }
        // Block until response or error (None) arrives
        let (request, response) = rx.recv().unwrap();
        // We received some response or error, decrement number of pending requests
        request_queue.decr_pending(&request);
        if let Some(ref mut urls_file) = urls_file {
            let timestamp = 0; // TODO
            let status = if let Some(ref response) = response {
                response.status.to_string()
            } else {
                "-".to_string()
            };
            // TODO - make it really csv
            write!(urls_file, "{},{},{}\n", timestamp, status, request.url).unwrap();
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
