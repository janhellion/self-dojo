use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod db;
mod ollama;
mod parse;
mod sync;
mod types;

const DEFAULT_DB: &str = ".local/share/self-dojo/dojo.db";
const DEFAULT_MODEL: &str = "llama3.2";

#[derive(Parser)]
#[command(name = "dojo-engine", about = "self-dojo journaling engine")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to the SQLite database
    #[arg(short = 'd', long = "db", global = true)]
    db: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Register and parse a new journal entry
    New {
        /// Path to the markdown file
        file: String,
        /// Energy level (1-10)
        #[arg(long)]
        energy: Option<u8>,
        /// Mood
        #[arg(long)]
        mood: Option<String>,
        /// Prompt answered
        #[arg(long)]
        prompt: Option<String>,
    },
    /// Re-parse an existing entry (update markers)
    Parse {
        /// Path to the markdown file
        file: String,
    },
    /// Manually link a patch to a weakness
    Link {
        /// Patch marker ID
        patch_id: i64,
        /// Weakness marker ID
        weakness_id: i64,
    },
    /// Ask Ollama to suggest a weakness match for a patch
    Suggest {
        /// The patch tag text
        text: String,
    },
    /// List all unresolved weaknesses
    Weaknesses {
        /// Output as JSON (for bash consumption)
        #[arg(long)]
        json: bool,
    },
    /// Print chronological entry tree
    Log,
    /// Print statistics
    Deck,
    /// Sync entries to CouchDB
    Sync,
    /// Check Ollama connection
    Check,
    /// Remove an entry by file path
    Remove {
        file: String,
    },
}

fn get_db_path(cli_db: Option<&String>) -> PathBuf {
    if let Some(p) = cli_db {
        PathBuf::from(p)
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(&home).join(DEFAULT_DB)
    }
}

fn main() {
    let cli = Cli::parse();
    let db_path = get_db_path(cli.db.as_ref());

    // Ensure data directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!("Warning: couldn't create {}: {}", parent.display(), e);
        });
    }

    let database = match db::Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Database error: {}", e);
            std::process::exit(1);
        }
    };

    match &cli.command {
        Commands::New {
            file,
            energy,
            mood,
            prompt,
        } => cmd_new(&database, file, *energy, mood.as_deref(), prompt.as_deref()),
        Commands::Parse { file } => cmd_parse(&database, file),
        Commands::Link {
            patch_id,
            weakness_id,
        } => cmd_link(&database, *patch_id, *weakness_id),
        Commands::Suggest { text } => cmd_suggest(&database, text),
        Commands::Weaknesses { json } => cmd_weaknesses(&database, *json),
        Commands::Log => cmd_log(&database),
        Commands::Deck => cmd_deck(&database),
        Commands::Sync => cmd_sync(&database),
        Commands::Check => cmd_check(),
        Commands::Remove { file } => cmd_remove(&database, file),
    }
}

fn cmd_new(
    database: &db::Database,
    file: &str,
    energy: Option<u8>,
    mood: Option<&str>,
    prompt: Option<&str>,
) {
    if database.entry_exists(file) {
        // Entry already registered — just re-parse for markers
        cmd_parse(database, file);
        return;
    }

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", file, e);
            std::process::exit(1);
        }
    };

    // Try to read frontmatter for fallback values
    let (meta, _body) = parse::parse_frontmatter(&content);

    let created_at = meta
        .get("date")
        .cloned()
        .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());

    let energy_val = energy
        .or_else(|| meta.get("energy").and_then(|v| v.parse().ok()))
        .unwrap_or(5);

    let mood_val = mood
        .map(|s| s.to_string())
        .or_else(|| meta.get("mood").cloned())
        .unwrap_or_else(|| "neutral".to_string());

    let prompt_val = prompt
        .map(|s| s.to_string())
        .or_else(|| meta.get("prompt").cloned())
        .unwrap_or_default();

    let entry = types::Entry {
        id: None,
        created_at,
        file_path: file.to_string(),
        energy: energy_val,
        mood: mood_val,
        prompt: prompt_val,
    };

    let entry_id = match database.insert_entry(&entry) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to save entry: {}", e);
            std::process::exit(1);
        }
    };

    let markers = parse::extract_markers(&content, entry_id);
    let mut saved_markers = Vec::new();
    for marker in &markers {
        match database.insert_marker(marker) {
            Ok(marker_id) => {
                if let Ok(Some(full_marker)) = database.marker_by_id(marker_id) {
                    saved_markers.push(full_marker);
                }
            }
            Err(e) => eprintln!("Warning: failed to save marker: {}", e),
        }
    }

    // Output structured data for bash
    let result = serde_json::json!({
        "entry_id": entry_id,
        "file": file,
        "markers": saved_markers.iter().map(|m| serde_json::json!({
            "id": m.id,
            "kind": m.kind.as_str(),
            "text": m.text
        })).collect::<Vec<_>>(),
        "weakness_count": saved_markers.iter().filter(|m| m.kind == types::MarkerKind::Weakness).count(),
        "patch_count": saved_markers.iter().filter(|m| m.kind == types::MarkerKind::Patch).count(),
    });

    println!("{}", serde_json::to_string(&result).unwrap());
}

fn cmd_parse(database: &db::Database, file: &str) {
    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading {}: {}", file, e);
            return;
        }
    };

    // Find entry by file path
    let entries = database.all_entries().unwrap_or_default();
    let entry = match entries.into_iter().find(|e| e.file_path == file) {
        Some(e) => e,
        None => {
            eprintln!("Entry not found for: {}", file);
            return;
        }
    };

    let markers = parse::extract_markers(&content, entry.id.unwrap());
    for marker in &markers {
        if let Err(e) = database.insert_marker(marker) {
            eprintln!("Warning: failed to save marker: {}", e);
        }
    }

    println!(
        "{}",
        serde_json::json!({
            "entry_id": entry.id,
            "file": file,
            "markers_found": markers.len(),
        })
    );
}

fn cmd_link(database: &db::Database, patch_id: i64, weakness_id: i64) {
    let bridge = types::Bridge {
        id: None,
        patch_id,
        weakness_id,
    };
    match database.insert_bridge(&bridge) {
        Ok(_) => println!("Linked patch #{} to weakness #{}", patch_id, weakness_id),
        Err(e) => eprintln!("Failed to link: {}", e),
    }
}

fn cmd_suggest(database: &db::Database, text: &str) {
    let weaknesses = match database.unresolved_weaknesses() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error loading weaknesses: {}", e);
            return;
        }
    };

    if weaknesses.is_empty() {
        println!("NO_WEAKNESSES");
        return;
    }

    let model = std::env::var("DOJO_OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    match ollama::suggest_match(text, &weaknesses, &model) {
        Some((id, match_text)) => {
            println!(
                "{}",
                serde_json::json!({
                    "id": id,
                    "text": match_text
                })
            );
        }
        None => {
            println!("NO_MATCH");
        }
    }
}

fn cmd_weaknesses(database: &db::Database, json_output: bool) {
    let weaknesses = match database.unresolved_weaknesses() {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    if json_output {
        println!("{}", serde_json::to_string(&weaknesses).unwrap());
    } else {
        for w in &weaknesses {
            println!("{}|{}|{}", w.id, w.text, w.count);
        }
    }
}

fn cmd_log(database: &db::Database) {
    let entries = match database.all_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error: {}", e);
            return;
        }
    };

    if entries.is_empty() {
        println!("NO_ENTRIES");
        return;
    }

    // Group by date
    let mut by_date: std::collections::BTreeMap<String, Vec<&types::Entry>> =
        std::collections::BTreeMap::new();
    for entry in &entries {
        by_date
            .entry(entry.created_at.clone())
            .or_default()
            .push(entry);
    }

    for (date, day_entries) in by_date.iter().rev() {
        println!("{}", date);
        for entry in day_entries {
            let markers = database
                .all_markers_for_entry(entry.id.unwrap())
                .unwrap_or_default();
            let ws: Vec<&str> = markers
                .iter()
                .filter(|m| m.kind == types::MarkerKind::Weakness)
                .map(|m| m.text.as_str())
                .collect();
            let ps: Vec<&str> = markers
                .iter()
                .filter(|m| m.kind == types::MarkerKind::Patch)
                .map(|m| m.text.as_str())
                .collect();

            print!(
                "  · {} · {} · {}",
                entry.mood, entry.energy, entry.prompt
            );
            if !ws.is_empty() {
                print!(" · weakness({})", ws.join(", "));
            }
            if !ps.is_empty() {
                print!(" · patch({})", ps.join(", "));
            }
            // Show linked pairs
            if let Ok(pairs) = database.linked_pairs_for_entry(entry.id.unwrap()) {
                if !pairs.is_empty() {
                    let linked: Vec<String> = pairs.iter().map(|(p, w)| format!("{} → {}", p, w)).collect();
                    print!(" · linked({})", linked.join(", "));
                }
            }
            println!();
            println!("    {}", entry.file_path);
        }
    }
}

fn cmd_deck(database: &db::Database) {
    match database.stats() {
        Ok((total, avg_energy, (high_count, low_count), top_weaknesses)) => {
            println!(
                "{}",
                serde_json::json!({
                    "total_entries": total,
                    "avg_energy": format!("{:.1}", avg_energy),
                    "high_ratio": format!("{}/{}", high_count, total),
                    "low_ratio": format!("{}/{}", low_count, total),
                    "top_weaknesses": top_weaknesses.iter().map(|w| serde_json::json!({
                        "text": w.text,
                        "count": w.count
                    })).collect::<Vec<_>>()
                })
            );
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}

fn cmd_sync(database: &db::Database) {
    let couch_url = sync::get_couch_url();

    if !sync::check_couch(&couch_url) {
        eprintln!("CouchDB not reachable at {}", couch_url);
        std::process::exit(1);
    }

    let entries = match database.all_entries() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Error loading entries: {}", e);
            return;
        }
    };

    let mut synced = 0;
    let mut failed = 0;

    for entry in &entries {
        if !std::path::Path::new(&entry.file_path).exists() {
            failed += 1;
            continue;
        }

        // Build JSON of this entry's SQLite rows
        let entry_id = entry.id.unwrap();
        let markers = database.all_markers_for_entry(entry_id).unwrap_or_default();
        let markers_json: Vec<serde_json::Value> = markers
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "entry_id": m.entry_id,
                    "kind": m.kind.as_str(),
                    "text": m.text,
                    "resolved": m.resolved,
                })
            })
            .collect();

        let bridges_json: Vec<serde_json::Value> = markers
            .iter()
            .filter(|m| m.kind == types::MarkerKind::Weakness && m.resolved)
            .filter_map(|m| {
                // Find bridges where this weakness is linked
                // We approximate by including the weakness_id as reference
                m.id.map(|wid| {
                    serde_json::json!({
                        "weakness_id": wid,
                        "marker_text": m.text,
                    })
                })
            })
            .collect();

        let entry_json = serde_json::json!({
            "entry": {
                "id": entry_id,
                "created_at": entry.created_at,
                "file_path": entry.file_path,
                "energy": entry.energy,
                "mood": entry.mood,
                "prompt": entry.prompt,
            },
            "markers": markers_json,
            "bridges": bridges_json,
        });

        match sync::push_to_couch(&entry.file_path, &entry_json, &couch_url) {
            Ok(_) => synced += 1,
            Err(e) => {
                eprintln!("  Failed to sync {}: {}", entry.file_path, e);
                failed += 1;
            }
        }
    }

    println!(
        "{}",
        serde_json::json!({
            "synced": synced,
            "failed": failed,
            "couch_url": couch_url
        })
    );
}

fn cmd_check() {
    let ollama_ok = ollama::check_connection();
    let couch_url = sync::get_couch_url();
    let couch_ok = sync::check_couch(&couch_url);

    let models = if ollama_ok {
        ollama::list_models()
    } else {
        vec![]
    };

    println!(
        "{}",
        serde_json::json!({
            "ollama": ollama_ok,
            "couchdb": couch_ok,
            "couch_url": couch_url,
            "models": models
        })
    );
}

fn cmd_remove(database: &db::Database, file: &str) {
    match database.remove_entry(file) {
        Ok(_) => println!("Removed entry for: {}", file),
        Err(e) => eprintln!("Failed to remove entry: {}", e),
    }
}
