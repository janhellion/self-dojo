use rusqlite::{params, Connection, Result};
use std::path::Path;

use crate::types::{Bridge, Entry, Marker, MarkerKind, WeaknessSummary};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Database { conn };
        db.ensure_schema()?;
        Ok(db)
    }

    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                energy INTEGER NOT NULL DEFAULT 5,
                mood TEXT NOT NULL DEFAULT 'neutral',
                prompt TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS markers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entry_id INTEGER NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK(kind IN ('weakness', 'patch')),
                text TEXT NOT NULL,
                resolved INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS bridges (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                patch_id INTEGER NOT NULL REFERENCES markers(id) ON DELETE CASCADE,
                weakness_id INTEGER NOT NULL REFERENCES markers(id) ON DELETE CASCADE,
                UNIQUE(patch_id, weakness_id)
            );
            CREATE INDEX IF NOT EXISTS idx_markers_kind_resolved ON markers(kind, resolved);
            CREATE INDEX IF NOT EXISTS idx_markers_entry ON markers(entry_id);
            CREATE INDEX IF NOT EXISTS idx_entries_created ON entries(created_at);",
        )?;
        Ok(())
    }

    pub fn insert_entry(&self, entry: &Entry) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO entries (created_at, file_path, energy, mood, prompt)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                entry.created_at,
                entry.file_path,
                entry.energy as i64,
                entry.mood,
                entry.prompt
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn entry_exists(&self, file_path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM entries WHERE file_path = ?1",
                params![file_path],
                |row| row.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0
    }

    pub fn insert_marker(&self, marker: &Marker) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO markers (entry_id, kind, text, resolved)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                marker.entry_id,
                marker.kind.as_str(),
                marker.text,
                marker.resolved as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn marker_by_id(&self, id: i64) -> Result<Option<Marker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_id, kind, text, resolved FROM markers WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![id], |row| {
            Ok(Marker {
                id: Some(row.get(0)?),
                entry_id: row.get(1)?,
                kind: MarkerKind::from_str(&row.get::<_, String>(2)?).unwrap_or(MarkerKind::Weakness),
                text: row.get(3)?,
                resolved: row.get::<_, i64>(4)? != 0,
            })
        })?;
        match rows.next() {
            Some(Ok(m)) => Ok(Some(m)),
            _ => Ok(None),
        }
    }

    pub fn insert_bridge(&self, bridge: &Bridge) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO bridges (patch_id, weakness_id) VALUES (?1, ?2)",
            params![bridge.patch_id, bridge.weakness_id],
        )?;
        // Mark both as resolved
        self.conn.execute(
            "UPDATE markers SET resolved = 1 WHERE id IN (?1, ?2)",
            params![bridge.patch_id, bridge.weakness_id],
        )?;
        Ok(())
    }

    pub fn unresolved_weaknesses(&self) -> Result<Vec<WeaknessSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.text, COUNT(DISTINCT e.id) as count
             FROM markers m
             JOIN entries e ON e.id = m.entry_id
             WHERE m.kind = 'weakness' AND m.resolved = 0
             GROUP BY m.id
             ORDER BY e.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(WeaknessSummary {
                id: row.get(0)?,
                text: row.get(1)?,
                count: row.get(2)?,
                unresolved: true,
            })
        })?;
        rows.collect()
    }

    pub fn all_markers_for_entry(&self, entry_id: i64) -> Result<Vec<Marker>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, entry_id, kind, text, resolved FROM markers WHERE entry_id = ?1
             ORDER BY id",
        )?;
        let rows = stmt.query_map(params![entry_id], |row| {
            Ok(Marker {
                id: Some(row.get(0)?),
                entry_id: row.get(1)?,
                kind: MarkerKind::from_str(&row.get::<_, String>(2)?).unwrap_or(MarkerKind::Weakness),
                text: row.get(3)?,
                resolved: row.get::<_, i64>(4)? != 0,
            })
        })?;
        rows.collect()
    }

    pub fn all_entries(&self) -> Result<Vec<Entry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, file_path, energy, mood, prompt
             FROM entries ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Entry {
                id: Some(row.get(0)?),
                created_at: row.get(1)?,
                file_path: row.get(2)?,
                energy: row.get::<_, i64>(3)? as u8,
                mood: row.get(4)?,
                prompt: row.get(5)?,
            })
        })?;
        rows.collect()
    }

    pub fn stats(&self) -> Result<(i64, f64, (i64, i64), Vec<WeaknessSummary>)> {
        let total: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entries", [], |row| row.get(0))?;

        let avg_energy: f64 = self
            .conn
            .query_row(
                "SELECT COALESCE(AVG(CAST(energy AS REAL)), 0.0) FROM entries",
                [],
                |row| row.get(0),
            )?;

        let high_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE energy >= 7",
            [],
            |row| row.get(0),
        )?;

        let low_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM entries WHERE energy <= 3",
            [],
            |row| row.get(0),
        )?;

        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.text, COUNT(DISTINCT b.id) as count, 0
             FROM markers m
             JOIN bridges b ON b.weakness_id = m.id
             WHERE m.kind = 'weakness'
             GROUP BY m.id
             ORDER BY count DESC
             LIMIT 10",
        )?;
        let top_weaknesses = stmt
            .query_map([], |row| {
                Ok(WeaknessSummary {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    count: row.get(2)?,
                    unresolved: false,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok((total, avg_energy, (high_count, low_count), top_weaknesses))
    }

    /// Get linked weakness texts for patches in this entry
    pub fn linked_pairs_for_entry(&self, entry_id: i64) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT m1.text, m2.text FROM bridges b JOIN markers m1 ON b.patch_id = m1.id JOIN markers m2 ON b.weakness_id = m2.id WHERE m1.entry_id = ?1"
        )?;
        let rows = stmt.query_map(params![entry_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect()
    }

    pub fn remove_entry(&self, file_path: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM entries WHERE file_path = ?1",
            params![file_path],
        )?;
        Ok(())
    }
}
