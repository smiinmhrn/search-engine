use crate::indexer::IndexStore;
use crate::search::{search, suggest_terms};
use actix_web::{middleware::Logger, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub index: Arc<IndexStore>,
}

#[derive(Deserialize)]
pub struct Query {
    q: String,
    page: Option<usize>,
}

pub async fn run_server(index: IndexStore, host: String) -> anyhow::Result<()> {
    let data = web::Data::new(AppState {
        index: Arc::new(index),
    });

    println!("🚀 Server starting at http://{}", host);

    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(Logger::default())
            .route("/", web::get().to(index_page))
            .route("/search", web::get().to(search_handler))
    })
    .bind(host)?
    .run()
    .await?;
    Ok(())
}

async fn index_page() -> impl Responder {
    let html = r#"
    <!DOCTYPE html>
    <html lang="fa" dir="rtl">
    <head>
        <meta charset='utf-8'>
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Search Engine | موتور جستجو</title>
        <style>
            :root { --primary: #2563eb; --bg: #ffffff; --text: #1f2937; }
            body { font-family: 'Segoe UI', Tahoma, sans-serif; background-color: #f8fafc; margin:0; display: flex; align-items: center; justify-content: center; height: 100vh; color: var(--text); }
            .container { text-align: center; width: 100%; max-width: 600px; padding: 20px; margin-top: -10vh; }
            h1 { font-size: 3.5rem; margin-bottom: 1.5rem; color: var(--primary); font-weight: 900; letter-spacing: -1px; }
            .search-box { background: white; padding: 8px; border-radius: 99px; box-shadow: 0 4px 20px rgba(0,0,0,0.08); display: flex; border: 1px solid #e2e8f0; transition: 0.3s; }
            .search-box:focus-within { box-shadow: 0 10px 25px rgba(37,99,235,0.15); border-color: var(--primary); }
            input[type="text"] { flex: 1; border: none; padding: 12px 25px; font-size: 18px; outline: none; background: transparent; }
            button { background: var(--primary); color: white; border: none; padding: 12px 35px; border-radius: 99px; font-size: 16px; cursor: pointer; transition: 0.2s; font-weight: bold; }
            button:hover { background: #1d4ed8; transform: scale(1.05); }
            .info { margin-top: 25px; color: #64748b; font-size: 0.9rem; }
        </style>
    </head>
    <body>
        <div class="container">
            <h1>Search Engine</h1>
            <form action="/search" method="get" class="search-box">
                <input type="text" name="q" placeholder="جستجو کنید..." required autofocus />
                <button type="submit">search</button>
            </form>
            <div class="info">طراحی شده با Rust</div>
        </div>
    </body>
    </html>
    "#;
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}

fn render_pagination(q: &str, page: usize, total_pages: usize) -> String {
    if total_pages <= 1 {
        return String::new();
    }
    let max_visible = 5;
    let mut html = String::from("<div class='pagination'>");
    html.push_str(r#"<style>
        .pagination { display: flex; justify-content: center; gap: 8px; margin-top: 40px; padding-bottom: 50px; flex-wrap: wrap; }
        .page-link, .page-current { min-width: 45px; height: 45px; display: flex; align-items: center; justify-content: center; text-decoration: none; border-radius: 12px; font-weight: bold; font-size: 14px; transition: 0.2s; }
        .page-link { background: white; color: #1e293b; border: 1px solid #e2e8f0; }
        .page-link:hover { background: #2563eb; color: white; border-color: #2563eb; transform: translateY(-2px); }
        .page-current { background: #2563eb; color: white; box-shadow: 0 4px 10px rgba(37,99,235,0.3); }
    </style>"#);

    let half = max_visible / 2;
    let start_page = if page > half {
        (page - half).min(total_pages.saturating_sub(max_visible) + 1)
    } else {
        1
    };
    let end_page = (start_page + max_visible - 1).min(total_pages);

    if page > 1 {
        html.push_str(&format!(
            "<a class='page-link' href='/search?q={}&page={}'>قبلی</a>",
            html_escape::encode_text(q),
            page - 1
        ));
    }
    for p in start_page..=end_page {
        if p == page {
            html.push_str(&format!("<span class='page-current'>{}</span>", p));
        } else {
            html.push_str(&format!(
                "<a class='page-link' href='/search?q={}&page={}'>{}</a>",
                html_escape::encode_text(q),
                p,
                p
            ));
        }
    }
    if page < total_pages {
        html.push_str(&format!(
            "<a class='page-link' href='/search?q={}&page={}'>بعدی</a>",
            html_escape::encode_text(q),
            page + 1
        ));
    }
    html.push_str("</div>");
    html
}

async fn search_handler(
    data: web::Data<AppState>,
    web::Query(query): web::Query<Query>,
) -> impl Responder {
    let q = query.q.trim();
    let page_size = 10;
    let page = query.page.unwrap_or(1).max(1);
    let start_time = Instant::now();

    // مجموعه کلماتی که باید هایلایت شوند
    let mut highlight_terms: HashSet<String> = crate::normalize::tokenize(q).into_iter().collect();

    let mut all_results: Vec<(usize, f64)> = search(&data.index, q, 1000);
    let mut suggestion_html = String::new();
    let mut extra_results: Vec<(usize, f64)> = Vec::new();
    let tokens = crate::normalize::tokenize(q);
    let mut suggs_list = Vec::new();

    for t in tokens.iter() {
        let s = suggest_terms(&data.index, t, 2, 3);
        if !s.is_empty() {
            suggs_list.push(format!("<b>{}</b> &rarr; {}", t, s.join(", ")));
            for sug in s.iter() {
                highlight_terms.insert(sug.clone());
                extra_results.extend(search(&data.index, sug, 30));
            }
        }
    }

    if !suggs_list.is_empty() {
        suggestion_html = format!(
            "<div class='suggestion-box'>🔍 شاید منظور شما این بود: {}</div>",
            suggs_list.join(" | ")
        );
    }

    let mut seen = HashSet::new();
    all_results.retain(|(id, _)| seen.insert(*id));
    for (id, score) in extra_results {
        if seen.insert(id) {
            all_results.push((id, score));
        }
    }

    let total_results = all_results.len();
    let total_pages = (total_results + page_size - 1) / page_size;
    let start_idx = (page - 1) * page_size;
    let end_idx = (start_idx + page_size).min(total_results);
    let current_results = if start_idx < total_results {
        &all_results[start_idx..end_idx]
    } else {
        &[]
    };
    let elapsed = start_time.elapsed().as_secs_f64();

    let mut body = String::new();
    body.push_str(r#"<!DOCTYPE html><html lang="fa" dir="rtl"><head><meta charset='utf-8'><meta name="viewport" 
    content="width=device-width, initial-scale=1.0"><title>نتایج جستجو | Search Engine</title><style>:root
     { --primary: #2563eb; --text-main: #1e293b; --text-muted: #64748b; } body { font-family: 'Tahoma',
       sans-serif; direction: rtl; background-color:#f1f5f9; margin:0; color: var(--text-main); line-height: 1.6; } 
       header { background: white; padding: 12px 5%; border-bottom: 1px solid #e2e8f0; position: sticky; top: 0; z-index: 100;
        display: flex; align-items: center; gap: 25px; box-shadow: 0 2px 10px rgba(0,0,0,0.02); } 
        .logo { font-weight: 900; color: var(--primary); text-decoration: none; font-size: 20px; min-width: 140px; letter-spacing: -0.5px; } 
    .search-form { display: flex; flex: 1; max-width: 600px; background: #f8fafc; border-radius: 10px; border: 1px solid #cbd5e1; overflow:
 hidden; transition: 0.2s; } .search-form:focus-within { border-color: var(--primary); box-shadow: 0 0 0 3px rgba(37,99,235,0.1); } 
 .search-form input { flex: 1; border: none; padding: 10px 15px; background: transparent; outline: none; font-size: 15px; } 
.search-form button { background: var(--primary); color: white; border: none; padding: 0 20px; cursor: pointer; font-weight: bold; }
 main { padding: 20px 5%; max-width: 900px; margin: auto; } .stats { font-size: 13px; color: var(--text-muted); margin-bottom: 
 15px; padding-right: 5px; } ol { list-style: none; padding: 0; } li { background: white; margin-bottom: 16px; padding: 20px; border-radius: 
 12px; border: 1px solid #e2e8f0; transition: 0.3s; } li:hover { box-shadow: 0 10px 20px rgba(0,0,0,0.05); transform: translateY(-2px); } 
 li a { color: var(--primary); font-size: 18px; text-decoration: none; font-weight: 600; display: block; margin-bottom: 5px; } li a:hover 
 { text-decoration: underline; } .meta-info { display: flex; align-items: center; gap: 10px; margin-bottom: 8px; }
   .score-badge { color: #10b981; font-weight: bold; background: #ecfdf5; padding: 2px 10px; border-radius: 6px; 
font-size: 11px; border: 1px solid #d1fae5; } .snippet { color: #475569; font-size: 14px; overflow: hidden; text-overflow: 
ellipsis; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; } mark { background-color: #fef08a; color:
 #854d0e; padding: 0 2px; border-radius: 3px; font-weight: 600; } .suggestion-box { background: #eff6ff; color: #1e40af; padding: 
 15px; border-radius: 10px; margin-bottom: 20px; font-size: 14px; border: 1px solid #bfdbfe; }</style></head><body><header><a href="/"
  class="logo">Search Engine</a><form action="/search" method="get" class="search-form"><input type="text" name="q" value="{QUERY}"
   id="nav-input" /><button type="submit">search</button></form></header><main>"#);

    let final_body = body.replace("{QUERY}", &html_escape::encode_text(q));
    let mut results_html = String::new();
    results_html.push_str(&format!(
        "<div class='stats'>حدود {} نتیجه پیدا شد ({:.4} ثانیه)</div>{}",
        total_results, elapsed, suggestion_html
    ));
    results_html.push_str("<ol>");

    for (doc_id, score) in current_results {
        let meta = &data.index.docs[*doc_id];
        let body_text = &meta.body;

        let mut first_match_pos = 0;
        let body_lower = body_text.to_lowercase();
        for term in &highlight_terms {
            if let Some(pos) = body_lower.find(&term.to_lowercase()) {
                first_match_pos = pos;
                break;
            }
        }

        let snippet_raw: String = {
            let start_char_idx = body_text[..first_match_pos]
                .chars()
                .count()
                .saturating_sub(60);
            body_text.chars().skip(start_char_idx).take(300).collect()
        };

        let mut highlighted_snippet = html_escape::encode_text(&snippet_raw).to_string();
        for term in &highlight_terms {
            if term.chars().count() > 1 {
                let escaped_term = html_escape::encode_text(term).to_string();
                let highlight_tag = format!("<mark>{}</mark>", escaped_term);
                highlighted_snippet = highlighted_snippet.replace(&escaped_term, &highlight_tag);
            }
        }

        results_html.push_str(&format!(
            r#"<li>
                <a href="{url}" target="_blank">{title}</a>
                <div class="meta-info"><span class="score-badge">Score: {score:.2}</span></div>
                <p class="snippet">{snippet}...</p>
            </li>"#,
            url = html_escape::encode_text(&meta.url),
            title = html_escape::encode_text(&meta.title),
            score = score,
            snippet = highlighted_snippet
        ));
    }

    results_html.push_str("</ol>");
    results_html.push_str(&render_pagination(q, page, total_pages));
    results_html.push_str("</main></body></html>");

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body([final_body, results_html].concat())
}
