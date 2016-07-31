#[macro_use] extern crate log;
extern crate env_logger;
extern crate crawler;

use std::env;
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::str;

use crawler::{crawl, Url, Settings};


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

    let settings = Settings::default();
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

    crawl(seeds, &settings);
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_seed() {
        assert_eq!(parse_seed("foo.com").unwrap(), "http://foo.com".parse().unwrap());
        assert_eq!(parse_seed("http://foo.com").unwrap(), "http://foo.com".parse().unwrap());
        assert_eq!(parse_seed("https://foo.com").unwrap(), "https://foo.com".parse().unwrap());
    }
}
