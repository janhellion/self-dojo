use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Option<i64>,
    pub created_at: String,
    pub file_path: String,
    pub energy: u8,
    pub mood: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Marker {
    pub id: Option<i64>,
    pub entry_id: i64,
    pub kind: MarkerKind,
    pub text: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MarkerKind {
    Weakness,
    Patch,
}

impl MarkerKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            MarkerKind::Weakness => "weakness",
            MarkerKind::Patch => "patch",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "weakness" => Some(MarkerKind::Weakness),
            "patch" => Some(MarkerKind::Patch),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bridge {
    pub id: Option<i64>,
    pub patch_id: i64,
    pub weakness_id: i64,
}

/// Energy-level counts used for high/low ratios.
pub struct EnergyStats {
    pub high_count: i64,  // 7–10
    pub low_count: i64,   // 1–3
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeaknessSummary {
    pub id: i64,
    pub text: String,
    pub count: i64,
    pub unresolved: bool,
}
