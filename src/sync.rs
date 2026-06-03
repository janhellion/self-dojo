use serde_json::{json, Value};
use std::path::Path;

const DEFAULT_COUCH_URL: &str = "http://localhost:5984/self-dojo";

/// Push a markdown file with its SQLite rows to CouchDB.
/// couch_url is the full database URL, e.g. https://user:pass@host/self-dojo
pub fn push_to_couch(file_path: &str, entry_json: &Value, couch_url: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = couch_url.trim_end_matches('/').to_string();

    let markdown_content = std::fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    let file_name = Path::new(file_path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let doc_id = format!("entry_{}", file_name.replace(['/', '\\', ' '], "_"));

    let doc = json!({
        "_id": doc_id,
        "type": "journal_entry",
        "content": markdown_content,
        "file_path": file_path,
        "sqlite_rows": entry_json,
    });

    // Create database if it doesn't exist (CouchDB returns 404 on PUT doc when DB missing)
    let db_check = client.get(&url).send();
    if let Ok(resp) = db_check {
        if resp.status().as_u16() == 404 {
            let _ = client.put(&url).send();
        }
    }

    let put_resp = client
        .put(format!("{}/{}", url, doc_id))
        .json(&doc)
        .send()
        .map_err(|e| format!("Failed to push to CouchDB: {}", e))?;

    if !put_resp.status().is_success() {
        return Err(format!("CouchDB push failed: {}", put_resp.status()));
    }

    Ok(())
}

/// Check if CouchDB is reachable at the given URL
pub fn check_couch(couch_url: &str) -> bool {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    match client.get(couch_url).send() {
        // Accept 200 (exists), 401 (needs auth), 404 (not created yet — push will create it)
        Ok(r) => r.status().is_success() || r.status().as_u16() == 401 || r.status().as_u16() == 404,
        Err(_) => false,
    }
}

/// Get CouchDB config from env var or default
pub fn get_couch_url() -> String {
    std::env::var("DOJO_COUCH_URL").unwrap_or_else(|_| DEFAULT_COUCH_URL.to_string())
}

/// List remote document IDs
pub fn list_remote_docs(couch_url: &str) -> Vec<String> {
    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let url = format!(
        "{}/_all_docs?include_docs=false",
        couch_url.trim_end_matches('/')
    );

    let resp: Value = match client.get(&url).send().ok() {
        Some(r) => r.json().unwrap_or(json!(null)),
        None => return vec![],
    };

    resp["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r["id"].as_str().map(|s| s.to_string()))
                .filter(|id| !id.starts_with('_'))
                .collect()
        })
        .unwrap_or_default()
}

/// Fetch a single document from CouchDB by ID. Returns its JSON content.
pub fn fetch_doc(couch_url: &str, doc_id: &str) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let url = format!("{}/{}", couch_url.trim_end_matches('/'), doc_id);
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("GET failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json().map_err(|e| format!("JSON parse: {}", e))
}
