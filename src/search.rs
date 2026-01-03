use crate::indexer::IndexStore;
use crate::normalize::tokenize;
use std::collections::{HashMap, HashSet};

const K1: f64 = 1.2;
const B: f64 = 0.75;
const TITLE_WEIGHT: f64 = 5.0;

pub fn search(index: &IndexStore, query: &str, top_k: usize) -> Vec<(usize, f64)> {
    let qterms = tokenize(query);
    if qterms.is_empty() {
        return vec![];
    }

    let mut sets: Vec<HashSet<usize>> = Vec::new();
    for t in &qterms {
        if let Some(postings) = index.dict.get(t) {
            sets.push(postings.iter().map(|p| p.doc_id).collect());
        } else {
            return vec![];
        }
    }

    let candidates: HashSet<usize> = sets
        .into_iter()
        .reduce(|a, b| a.intersection(&b).cloned().collect())
        .unwrap_or_default();

    if candidates.is_empty() {
        return vec![];
    }

    let avg_len =
        index.docs.iter().map(|d| d.length).sum::<usize>() as f64 / index.doc_count.max(1) as f64;

    let mut scores: HashMap<usize, f64> = HashMap::new();

    for term in &qterms {
        if let Some(postings) = index.dict.get(term) {
            let df = postings.len() as f64;
            let idf = ((index.doc_count as f64 - df + 0.5) / (df + 0.5) + 1.0).ln();

            for p in postings {
                if !candidates.contains(&p.doc_id) {
                    continue;
                }

                let doc_len = index.docs[p.doc_id].length as f64;
                let tf = p.tf as f64;

                let denom = tf + K1 * (1.0 - B + B * doc_len / avg_len.max(1.0));
                let score = idf * ((tf * (K1 + 1.0)) / denom);

                *scores.entry(p.doc_id).or_insert(0.0) += score;
            }
        }
    }

    for &doc_id in &candidates {
        let title_tokens = tokenize(&index.docs[doc_id].title);
        let mut hits = 0;

        for t in &qterms {
            if title_tokens.contains(t) {
                hits += 1;
            }
        }

        if hits > 0 {
            *scores.entry(doc_id).or_insert(0.0) += hits as f64 * TITLE_WEIGHT;
        }
    }

    apply_proximity_boost(index, &qterms, &candidates, &mut scores);

    let mut results: Vec<_> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(top_k);
    results
}

fn apply_proximity_boost(
    index: &IndexStore,
    qterms: &[String],
    candidates: &HashSet<usize>,
    scores: &mut HashMap<usize, f64>,
) {
    if qterms.len() < 2 {
        return;
    }

    for &doc_id in candidates {
        let mut min_total_dist = 0;
        let mut pair_found = false;

        for pair in qterms.windows(2) {
            let p1 = index
                .dict
                .get(&pair[0])
                .and_then(|v| v.iter().find(|p| p.doc_id == doc_id));
            let p2 = index
                .dict
                .get(&pair[1])
                .and_then(|v| v.iter().find(|p| p.doc_id == doc_id));

            if let (Some(pos1), Some(pos2)) = (p1, p2) {
                let mut best_pair_dist = usize::MAX;
                for &a in &pos1.positions {
                    for &b in &pos2.positions {
                        let d = a.abs_diff(b);
                        if d < best_pair_dist {
                            best_pair_dist = d;
                        }
                    }
                }
                min_total_dist += best_pair_dist;
                pair_found = true;
            }
        }

        if pair_found {
            let boost = match min_total_dist {
                d if d <= qterms.len() - 1 => 5.0,
                d if d <= (qterms.len() - 1) * 2 => 2.5,
                d if d <= (qterms.len() - 1) * 5 => 1.0,
                _ => 0.0,
            };
            *scores.entry(doc_id).or_insert(0.0) += boost;
        }
    }
}

pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    let mut dp = vec![vec![0; m + 1]; n + 1];

    for i in 0..=n {
        dp[i][0] = i;
    }
    for j in 0..=m {
        dp[0][j] = j;
    }

    for i in 1..=n {
        for j in 1..=m {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);

            if i > 1
                && j > 1
                && a_chars[i - 1] == b_chars[j - 2]
                && a_chars[i - 2] == b_chars[j - 1]
            {
                dp[i][j] = dp[i][j].min(dp[i - 2][j - 2] + 1);
            }
        }
    }
    dp[n][m]
}

pub fn suggest_terms(
    index: &IndexStore,
    token: &str,
    max_dist: usize,
    max_suggestions: usize,
) -> Vec<String> {
    let mut cands: Vec<(String, f64)> = Vec::new();

    for (term, postings) in index.dict.iter() {
        let dist = damerau_levenshtein(term, token);
        if dist > max_dist {
            continue;
        }

        let df = postings.len() as f64;
        let score = -(dist as f64) * 3.0 + (df + 1.0).ln();
        cands.push((term.clone(), score));
    }

    cands.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    cands
        .into_iter()
        .take(max_suggestions)
        .map(|(t, _)| t)
        .collect()
}
