use regex::Regex;

use crate::types::{Marker, MarkerKind};

/// Parse a markdown entry file and extract @weakness[text] and @patch[text] tags.
pub fn extract_markers(content: &str, entry_id: i64) -> Vec<Marker> {
    let re = Regex::new(r"@(weakness|patch)\[([^\]]+)\]").unwrap();
    let mut markers = Vec::new();

    for cap in re.captures_iter(content) {
        let kind_str = &cap[1];
        let text = cap[2].trim().to_string();
        if text.is_empty() {
            continue;
        }
        let kind = match kind_str {
            "weakness" => MarkerKind::Weakness,
            "patch" => MarkerKind::Patch,
            _ => continue,
        };
        markers.push(Marker {
            id: None,
            entry_id,
            kind,
            text,
            resolved: false,
        });
    }

    markers
}

/// Parse YAML frontmatter from markdown content.
/// Returns (frontmatter_map, body_content).
pub fn parse_frontmatter(content: &str) -> (std::collections::HashMap<String, String>, &str) {
    let content = content.trim_start();
    let mut map = std::collections::HashMap::new();

    if !content.starts_with("---") {
        return (map, content);
    }

    let end = content[3..].find("---").map(|pos| pos + 3);
    let end = match end {
        Some(e) => e,
        None => return (map, content),
    };

    let front = &content[3..end];
    let body = content[end + 3..].trim_start();

    for line in front.lines() {
        let line = line.trim();
        if let Some(pos) = line.find(':') {
            let key = line[..pos].trim().to_lowercase();
            let value = line[pos + 1..].trim().to_string();
            map.insert(key, value);
        }
    }

    (map, body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_weakness_and_patch() {
        let content = "Today I struggled with @weakness[focus] but found @patch[pomodoro] helped.";
        let markers = extract_markers(content, 1);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].kind, MarkerKind::Weakness);
        assert_eq!(markers[0].text, "focus");
        assert_eq!(markers[1].kind, MarkerKind::Patch);
        assert_eq!(markers[1].text, "pomodoro");
    }

    #[test]
    fn test_parse_frontmatter() {
        let content = "---\ndate: 2026-06-02\nmood: tired\nenergy: 4\nprompt: What helped today?\n---\n\nBody text here...";
        let (meta, body) = parse_frontmatter(content);
        assert_eq!(meta.get("mood").unwrap(), "tired");
        assert_eq!(meta.get("energy").unwrap(), "4");
        assert_eq!(body, "Body text here...");
    }

    #[test]
    fn test_empty_markers() {
        let markers = extract_markers("No tags here at all.", 1);
        assert!(markers.is_empty());
    }
}
