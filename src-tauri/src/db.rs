use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

/// Bumped whenever the schema changes. Migrations 2 and 3 are reserved for
/// topics (M3) and the vector index (M5).
const SCHEMA_VERSION: i32 = 1;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL is what lets the indexer write while the UI reads, without a queue.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }
    if version < 1 {
        conn.execute_batch(SCHEMA_V1)?;
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// `remove_diacritics 2` is the reason searching "codigo" finds "código" — not
/// optional for a vault written in Portuguese.
const SCHEMA_V1: &str = r#"
CREATE TABLE notes (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL,
  folder        TEXT NOT NULL,
  mtime         INTEGER NOT NULL,
  content_hash  TEXT NOT NULL
);

CREATE TABLE assets (
  id       INTEGER PRIMARY KEY,
  note_id  INTEGER REFERENCES notes(id) ON DELETE SET NULL,
  path     TEXT NOT NULL UNIQUE,
  ocr_json TEXT,
  mtime    INTEGER NOT NULL
);

-- A chunk belongs to a note (kind='text') or to an image (kind='image').
-- Both live in the same table on purpose: one query finds prose and screenshots
-- alike, with no special case anywhere in the search path.
CREATE TABLE chunks (
  id            INTEGER PRIMARY KEY,
  note_id       INTEGER REFERENCES notes(id) ON DELETE CASCADE,
  asset_id      INTEGER REFERENCES assets(id) ON DELETE CASCADE,
  kind          TEXT NOT NULL CHECK (kind IN ('text', 'image')),
  heading_path  TEXT NOT NULL DEFAULT '',
  start_offset  INTEGER NOT NULL DEFAULT 0,
  end_offset    INTEGER NOT NULL DEFAULT 0,
  text          TEXT NOT NULL
);

CREATE INDEX chunks_note_idx  ON chunks(note_id);
CREATE INDEX chunks_asset_idx ON chunks(asset_id);

CREATE VIRTUAL TABLE chunks_fts USING fts5(
  text,
  content='chunks',
  content_rowid='id',
  tokenize="unicode61 remove_diacritics 2"
);

CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
  INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;

CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
END;

CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text) VALUES ('delete', old.id, old.text);
  INSERT INTO chunks_fts(rowid, text) VALUES (new.id, new.text);
END;
"#;
