//! User ⭐/📝. Backed up to `notes.json` to survive a DB rebuild (spec section 11).
use std::path::Path;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::Store;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub prompt_key: String,
    pub starred: bool,
    pub note: String,
    pub updated_at: String,
}

impl Store {
    pub fn set_note(&mut self, prompt_key: &str, starred: bool, note: &str, ts: &str) -> rusqlite::Result<()> {
        self.conn().execute(
            "INSERT INTO notes(prompt_key, starred, note, updated_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(prompt_key) DO UPDATE SET starred = excluded.starred, note = excluded.note, updated_at = excluded.updated_at",
            params![prompt_key, starred, note, ts],
        )?;
        Ok(())
    }

    pub fn get_note(&self, prompt_key: &str) -> rusqlite::Result<Option<Note>> {
        self.conn()
            .query_row("SELECT prompt_key, starred, note, updated_at FROM notes WHERE prompt_key = ?1", [prompt_key], row_to_note)
            .optional()
    }

    pub fn all_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let mut st = self.conn().prepare("SELECT prompt_key, starred, note, updated_at FROM notes ORDER BY prompt_key")?;
        let rows = st.query_map([], row_to_note)?;
        rows.collect()
    }
}

fn row_to_note(r: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note { prompt_key: r.get(0)?, starred: r.get(1)?, note: r.get(2)?, updated_at: r.get(3)? })
}

pub fn export_notes(store: &Store, path: &Path) -> anyhow::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&store.all_notes()?)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// Only add notes that don't exist yet; do not overwrite notes already in the DB. Returns the number of notes added.
pub fn import_notes(store: &mut Store, path: &Path) -> anyhow::Result<usize> {
    let Ok(raw) = std::fs::read(path) else { return Ok(0) };
    let notes: Vec<Note> = serde_json::from_slice(&raw)?;
    let mut added = 0;
    for n in notes {
        added += store.conn().execute(
            "INSERT OR IGNORE INTO notes(prompt_key, starred, note, updated_at) VALUES(?1, ?2, ?3, ?4)",
            params![n.prompt_key, n.starred, n.note, n.updated_at],
        )?;
    }
    Ok(added)
}
