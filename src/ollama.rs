use serde_json::{json, Value};

use crate::types::WeaknessSummary;

const OLLAMA_URL: &str = "http://localhost:11434/api/generate";

/// Ask Ollama to find the best match for a patch against existing unresolved weaknesses.
/// Returns (weakness_id, text) or None.
pub fn suggest_match(
    patch_text: &str,
    weaknesses: &[WeaknessSummary],
    model: &str,
) -> Option<(i64, String)> {
    if weaknesses.is_empty() {
        return None;
    }

    let weaknesses_list: Vec<&str> = weaknesses.iter().map(|w| w.text.as_str()).collect();
    let prompt = format!(
        "Given a journal entry's @patch tag \"{}\", which of these unresolved @weakness tags does it most likely address?\n\nWeaknesses:\n{}\n\nRespond with ONLY the exact text of the best matching weakness. If none match well, respond with \"NONE\".",
        patch_text,
        weaknesses_list.join("\n")
    );

    let body = json!({
        "model": model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "temperature": 0.3,
            "num_predict": 100
        }
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;

    let resp: Value = client
        .post(OLLAMA_URL)
        .json(&body)
        .send()
        .ok()?
        .json()
        .ok()?;

    let response_text = resp["response"].as_str()?.trim().to_string();

    if response_text.eq_ignore_ascii_case("NONE") || response_text.is_empty() {
        return None;
    }

    // Find the closest matching weakness by text
    let normalized = response_text.to_lowercase();
    for w in weaknesses {
        if w.text.to_lowercase() == normalized
            || normalized.contains(&w.text.to_lowercase())
            || w.text.to_lowercase().contains(&normalized)
        {
            return Some((w.id, w.text.clone()));
        }
    }

    // Fuzzy fallback: find the weakness with most word overlap
    let words: Vec<&str> = normalized.split_whitespace().collect();
    let best = weaknesses
        .iter()
        .max_by_key(|w| {
            let w_lower = w.text.to_lowercase();
            let w_words: Vec<&str> = w_lower.split_whitespace().collect();
            words.iter().filter(|w| w_words.contains(w)).count()
        })
        .map(|w| (w.id, w.text.clone()));

    best
}

/// Test connection to Ollama
pub fn check_connection() -> bool {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build();
    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.get("http://localhost:11434/api/tags").send() {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

/// List available models
pub fn list_models() -> Vec<String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let resp: Value = match client.get("http://localhost:11434/api/tags").send() {
        Ok(r) => match r.json() {
            Ok(v) => v,
            Err(_) => return vec![],
        },
        Err(_) => return vec![],
    };

    resp["models"]
        .as_array()
        .map(|models| {
            models
                .iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}
