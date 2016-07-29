use hyper::status::StatusCode;
use hyper::header::Headers;


#[derive(Debug, Clone)]
pub struct Response {
    pub status: StatusCode,
    pub headers: Headers,
    pub body: Option<Vec<u8>>
}
