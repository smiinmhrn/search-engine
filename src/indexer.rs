use crate::parser::Page;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Posting {
    pub doc_id: usize,
    pub tf: usize,
    pub positions: Vec<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DocMeta {
    pub url: String,
    pub title: String,
    pub body: String,
    pub length: usize,
}

#[derive(Serialize, Deserialize)]
pub struct IndexStore {
    pub dict: HashMap<String, Vec<Posting>>,
    pub docs: Vec<DocMeta>,
    pub doc_count: usize,
}

impl IndexStore {
    pub fn new() -> Self {
        IndexStore {
            dict: HashMap::new(),
            docs: Vec::new(),
            doc_count: 0,
        }
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let f = File::create(path)?;
        let mut bw = BufWriter::with_capacity(1024 * 1024, f);
        bincode::serialize_into(&mut bw, &self)?;
        Ok(())
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let f = File::open(path)?;
        let br = std::io::BufReader::new(f);
        let s: IndexStore = bincode::deserialize_from(br)?;
        Ok(s)
    }
}

pub fn build_index(input_dir: &Path, out: &Path, limit: Option<usize>) -> anyhow::Result<()> {
    let entries: Vec<_> = WalkDir::new(input_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .collect();

    println!("Found {} files", entries.len());
    let max_docs = limit.unwrap_or(entries.len());
    let selected = &entries[..entries.len().min(max_docs)];

    let processed_data: Vec<(DocMeta, HashMap<String, Vec<usize>>)> = selected
        .par_iter()
        .map(|entry| {
            let p = entry.path();
            let page = crate::parser::parse_html_file(p).unwrap_or_else(|_| Page {
                url: p.to_string_lossy().to_string(),
                title: "".into(),
                body: "".into(),
            });

            let title_tokens = crate::normalize::tokenize(&page.title);
            let body_tokens = crate::normalize::tokenize(&page.body);

            let mut all_tokens = title_tokens;
            all_tokens.extend(body_tokens.clone());
            let length = all_tokens.len();

            let mut pos_map: HashMap<String, Vec<usize>> = HashMap::with_capacity(length / 2);
            for (pos, term) in all_tokens.into_iter().enumerate() {
                pos_map.entry(term).or_default().push(pos);
            }

            let snippet: String = page.body.chars().take(500).collect();

            (
                DocMeta {
                    url: page.url,
                    title: page.title,
                    body: snippet,
                    length,
                },
                pos_map,
            )
        })
        .collect();

    let mut store = IndexStore::new();
    store.docs.reserve(processed_data.len());

    for (doc_id, (meta, pos_map)) in processed_data.into_iter().enumerate() {
        store.docs.push(meta);
        for (term, positions) in pos_map {
            store.dict.entry(term).or_default().push(Posting {
                doc_id,
                tf: positions.len(),
                positions,
            });
        }
    }

    store.doc_count = store.docs.len();

    store.dict.par_iter_mut().for_each(|(_, postings)| {
        postings.sort_by_key(|p| p.doc_id);
    });

    println!("Saving index to {:?}...", out);
    std::fs::create_dir_all(out.parent().unwrap_or(Path::new(".")))?;
    store.save(out)?;

    println!(
        "✅ Indexed {} docs, unique terms: {}",
        store.doc_count,
        store.dict.len()
    );
    Ok(())
}
