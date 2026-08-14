use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

/// Bumped whenever the schema changes. Migration 3 is reserved for the vector
/// index (M5).
const SCHEMA_VERSION: i32 = 2;

pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    // WAL is what lets the indexer write while the UI reads, without a queue.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // The indexer and the UI hold separate connections; a write that lands
    // mid-indexing should wait rather than come back as SQLITE_BUSY.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    migrate(&conn)?;
    Ok(conn)
}

/// Same schema, no file. Lets the search tests run against the real SQL rather
/// than a hand-written copy of it that could drift.
#[cfg(test)]
pub fn open_in_memory() -> Result<Connection> {
    let conn = Connection::open_in_memory()?;
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
    if version < 2 {
        conn.execute_batch(SCHEMA_V2)?;
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

/// Topics. A folder answers "where does this file live"; a topic answers "what
/// is this about", and one file can be in several. These two tables mirror the
/// manifests in `.topics/` — the manifests are the record, this is the index.
const SCHEMA_V2: &str = r#"
CREATE TABLE topics (
  slug  TEXT PRIMARY KEY,
  name  TEXT NOT NULL,
  icon  TEXT,
  color TEXT
);

-- `target_path` is a note or an image, so it stays a plain path rather than a
-- foreign key into either table.
CREATE TABLE topic_members (
  topic_slug  TEXT NOT NULL REFERENCES topics(slug) ON DELETE CASCADE,
  target_path TEXT NOT NULL,
  PRIMARY KEY (topic_slug, target_path)
);

CREATE INDEX topic_members_target_idx ON topic_members(target_path);
"#;
