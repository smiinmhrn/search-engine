use scraper::{Html, Selector};
use std::fs;

pub struct Page {
    pub url: String,
    pub title: String,
    pub body: String,
}

pub fn parse_html_file(path: &std::path::Path) -> anyhow::Result<Page> {
    let html = fs::read_to_string(path)?;
    parse_html(&html, &path.to_string_lossy())
}

pub fn parse_html(html: &str, url: &str) -> anyhow::Result<Page> {
    let document = Html::parse_document(html);
    let selector_title = Selector::parse("title").unwrap();
    let selector_body = Selector::parse("body").unwrap();

    let title = document
        .select(&selector_title)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "".into());

    let body = document
        .select(&selector_body)
        .next()
        .map(|n| n.text().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| document.root_element().text().collect::<Vec<_>>().join(" "));

    Ok(Page {
        url: url.to_string(),
        title,
        body,
    })
}
