use html5ever::tokenizer::{TokenSink, Token, Tokenizer, TokenizerOpts};
use html5ever::tendril::{StrTendril};
use hyper::Url;


struct LinkExtractor {
    links: Vec<StrTendril>
}

impl TokenSink for LinkExtractor {
    fn process_token(&mut self, token: Token) {
        if let Token::TagToken(tag) = token {
            if tag.name.eq_str_ignore_ascii_case("a") {
                for attr in tag.attrs {
                    if attr.name.local.eq_str_ignore_ascii_case("href") {
                        self.links.push(attr.value);
                    }
                }
            }
        }
    }
}

pub fn extract_links(body: &str, base_url: &Url) -> Vec<Url> {
    let mut tokenizer = Tokenizer::new(
        LinkExtractor{links: Vec::new()}, TokenizerOpts::default());
    tokenizer.feed(StrTendril::from(body));
    tokenizer.run();
    tokenizer.end();
    let link_extractor = tokenizer.unwrap();
    link_extractor.links.iter().filter_map(|href| {
        if let Ok(url) = base_url.join(href) {
            let supported_scheme = {
                let scheme = url.scheme();
                scheme == "http" || scheme == "https"
            };
            if supported_scheme { Some(url) } else { None }
        } else {
            None
        }
    }).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links() {
        let base_url = "http://foo.com/a/".parse().unwrap();
        let html = "<b><a href=\"../boo.txt\">a boo</a></b>\
                    <a name=\"foo\"></a>\
                    <a href=\"javascript:void(0)\"></a>\
                    <a href=\"ftp://foo.com\"></a>\
                    <a href=\"http://example.com/zoo\">a zoo</a>";
        let links = extract_links(&html, &base_url);
        assert_eq!(links, vec!["http://foo.com/boo.txt".parse().unwrap(),
                               "http://example.com/zoo".parse().unwrap()])
    }
}
