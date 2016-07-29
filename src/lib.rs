#![deny(warnings)]
#[macro_use] extern crate log;
extern crate html5ever;
extern crate hyper;
extern crate mime;
extern crate url;

mod crawl;
mod downloader;
mod link_extraction;
mod queue;
mod request;
mod response;
mod settings;
mod stats;

// Re-exports
pub use crawl::crawl;
pub use request::Request;
pub use hyper::Url;
pub use settings::Settings;
