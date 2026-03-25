use lajjzy_core::types::GraphData;

use crate::action::{Arity, CompletionItem};

pub(crate) const REVSET_FUNCTIONS: &[(&str, Arity)] = &[
    ("all", Arity::Nullary),
    ("ancestors", Arity::Required),
    ("author", Arity::Required),
    ("bookmarks", Arity::Optional),
    ("committer", Arity::Required),
    ("conflicts", Arity::Nullary),
    ("connected", Arity::Required),
    ("descendants", Arity::Required),
    ("description", Arity::Required),
    ("diff_contains", Arity::Required),
    ("empty", Arity::Nullary),
    ("file", Arity::Required),
    ("fork_point", Arity::Required),
    ("heads", Arity::Required),
    ("immutable", Arity::Nullary),
    ("mine", Arity::Nullary),
    ("none", Arity::Nullary),
    ("present", Arity::Required),
    ("remote_bookmarks", Arity::Optional),
    ("root", Arity::Nullary),
    ("roots", Arity::Required),
    ("tags", Arity::Optional),
    ("trunk", Arity::Nullary),
    ("visible_heads", Arity::Nullary),
];

pub(crate) fn is_revset_boundary(c: char) -> bool {
    matches!(c, '&' | '|' | '~' | '(' | ')' | ':' | '.' | ',' | '+' | '-')
        || c.is_ascii_whitespace()
}

pub(crate) fn extract_current_word(query: &str) -> (usize, &str) {
    let boundary = query.rfind(is_revset_boundary);
    match boundary {
        Some(pos) => (pos + 1, &query[pos + 1..]),
        None => (0, query),
    }
}

pub(crate) fn compute_completions(query: &str, graph: &GraphData) -> Vec<CompletionItem> {
    let (_, current_word) = extract_current_word(query);
    if current_word.is_empty() {
        return vec![];
    }
    let word_lower = current_word.to_lowercase();
    let mut results = Vec::new();

    // 1. Revset functions (ranked first)
    for &(name, arity) in REVSET_FUNCTIONS {
        if name.starts_with(&word_lower as &str) {
            let insert_text = match arity {
                Arity::Nullary => format!("{name}()"),
                Arity::Optional | Arity::Required => format!("{name}("),
            };
            results.push(CompletionItem {
                display_text: insert_text.clone(),
                insert_text,
            });
        }
    }

    // 2-3: Repo entities from node_indices (deterministic order)
    // Note: bare author names are NOT completed — they are only valid inside
    // author()/committer() functions, which is context-sensitive (deferred to M6).
    let mut bookmarks = Vec::new();
    let mut change_ids = Vec::new();
    for &idx in graph.node_indices() {
        if let Some(cid) = graph.lines[idx].change_id.as_deref()
            && let Some(detail) = graph.details.get(cid)
        {
            for bm in &detail.bookmarks {
                bookmarks.push(bm.as_str());
            }
            // Change IDs: require 2+ char prefix to reduce noise
            if word_lower.len() >= 2 && cid.to_lowercase().starts_with(&word_lower as &str) {
                let desc = if detail.description.is_empty() {
                    "(no description)".to_string()
                } else {
                    detail.description.clone()
                };
                change_ids.push(CompletionItem {
                    insert_text: cid.to_string(),
                    display_text: format!("{cid} \u{2014} {desc}"),
                });
            }
        }
    }

    // Bookmarks (ranked second)
    bookmarks.sort_unstable();
    bookmarks.dedup();
    for bm in bookmarks {
        if bm.to_lowercase().starts_with(&word_lower as &str) {
            results.push(CompletionItem {
                insert_text: bm.to_string(),
                display_text: bm.to_string(),
            });
        }
    }

    // Change IDs (ranked third)
    results.extend(change_ids);

    results.truncate(20);
    results
}
