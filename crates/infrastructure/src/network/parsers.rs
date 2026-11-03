use mediaatlas_domain::ProviderFailure;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn parse_json(body: &[u8]) -> Result<Value, ProviderFailure> {
    serde_json::from_slice(body).map_err(|_| ProviderFailure::ParseError)
}

pub struct HtmlDocument(Html);
pub fn parse_html(body: &str) -> HtmlDocument {
    HtmlDocument(Html::parse_document(body))
}
impl HtmlDocument {
    pub fn css_text(&self, selector: &str) -> Result<Vec<String>, ProviderFailure> {
        let selector = Selector::parse(selector).map_err(|_| ProviderFailure::ParseError)?;
        Ok(self
            .0
            .select(&selector)
            .map(|n| n.text().collect::<String>().trim().to_owned())
            .collect())
    }
    pub fn json_ld(&self) -> Result<Vec<Value>, ProviderFailure> {
        self.0
            .select(&Selector::parse("script[type='application/ld+json']").expect("fixed selector"))
            .map(|n| parse_json(n.inner_html().as_bytes()))
            .collect()
    }
    pub fn open_graph(&self) -> BTreeMap<String, String> {
        self.0
            .select(&Selector::parse("meta[property][content]").expect("fixed selector"))
            .filter_map(|n| {
                let key = n.value().attr("property")?;
                key.starts_with("og:").then(|| {
                    (
                        key.to_owned(),
                        n.value().attr("content").unwrap_or_default().to_owned(),
                    )
                })
            })
            .collect()
    }
    pub fn embedded_json(&self, id: &str) -> Result<Value, ProviderFailure> {
        self.0
            .select(&Selector::parse("script[id]").expect("fixed selector"))
            .find(|n| n.value().attr("id") == Some(id))
            .ok_or(ProviderFailure::NotFound)
            .and_then(|n| parse_json(n.inner_html().as_bytes()))
    }
}
