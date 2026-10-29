use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

/// Bumped whenever the schema changes.
const SCHEMA_VERSION: i32 = 8;

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
    if version < 3 {
        conn.execute_batch(SCHEMA_V3)?;
    }
    if version < 4 {
        conn.execute_batch(SCHEMA_V4)?;
    }
    if version < 5 {
        conn.execute_batch(SCHEMA_V5)?;
    }
    if version < 6 {
        conn.execute_batch(SCHEMA_V6)?;
    }
    if version < 7 {
        conn.execute_batch(SCHEMA_V7)?;
    }
    if version < 8 {
        conn.execute_batch(SCHEMA_V8)?;
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

/// The name of a note and where it lives are as much of an answer as its prose:
/// searching `fastify` should find `node-js-fastify.md` even when the word is
/// only in the file name. They are copied onto the chunk rather than joined at
/// query time because the FTS table indexes its content table by rowid.
///
/// Nothing is deleted here. Clearing `content_hash` is what makes the scanner
/// read every note again — the parser stopped emitting a chunk for a heading
/// with no body, and those are already in the index. `assets` keeps its OCR.
const SCHEMA_V3: &str = r#"
ALTER TABLE chunks ADD COLUMN title TEXT NOT NULL DEFAULT '';
ALTER TABLE chunks ADD COLUMN path  TEXT NOT NULL DEFAULT '';

UPDATE chunks SET
  title = COALESCE((SELECT n.title FROM notes n WHERE n.id = chunks.note_id), ''),
  path  = COALESCE(
            (SELECT n.path FROM notes  n WHERE n.id = chunks.note_id),
            (SELECT a.path FROM assets a WHERE a.id = chunks.asset_id), '');

DROP TRIGGER chunks_ai;
DROP TRIGGER chunks_ad;
DROP TRIGGER chunks_au;
DROP TABLE chunks_fts;

CREATE VIRTUAL TABLE chunks_fts USING fts5(
  text, title, path,
  content='chunks',
  content_rowid='id',
  tokenize="unicode61 remove_diacritics 2"
);

CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
  INSERT INTO chunks_fts(rowid, text, title, path)
  VALUES (new.id, new.text, new.title, new.path);
END;

CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text, title, path)
  VALUES ('delete', old.id, old.text, old.title, old.path);
END;

CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
  INSERT INTO chunks_fts(chunks_fts, rowid, text, title, path)
  VALUES ('delete', old.id, old.text, old.title, old.path);
  INSERT INTO chunks_fts(rowid, text, title, path)
  VALUES (new.id, new.text, new.title, new.path);
END;

INSERT INTO chunks_fts(chunks_fts) VALUES('rebuild');

UPDATE notes SET content_hash = '';
"#;

/// Passages by meaning. Optional by nature: a vault without them searches
/// exactly as it did before.
///
/// The vector is a blob of little-endian f32 rather than a table of numbers —
/// nothing queries inside it, the whole thing is read at once and compared in
/// memory. `text_hash` is the opening of the passage, so a rewritten note
/// leaves its old vector visibly stale instead of quietly wrong.
const SCHEMA_V4: &str = r#"
CREATE TABLE embeddings (
  chunk_id   INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
  vector     BLOB NOT NULL,
  text_hash  TEXT NOT NULL
);
"#;

/// Embedding moved from Gemini to a model running on this machine, and the two
/// produce vectors of different widths — 768 against 384. A 768-wide vector is
/// not stale data to be refreshed, it is a measurement in another unit, so the
/// table is emptied rather than migrated. `pending` then lists every passage
/// again on its own, with no migration code to maintain.
const SCHEMA_V5: &str = r#"
DELETE FROM embeddings;
"#;

/// `assets.note_id` is where an image's note is recorded; `chunks.note_id` is the
/// copy the search groups by, and the two were allowed to drift — a note indexed
/// after the image it embeds updated the asset and left the chunk behind. The
/// result was the same screenshot twice: once inside the note, once beside it.
///
/// The writes no longer drift. This is the once-over for what was indexed while
/// they did, so nothing has to be scanned or read again.
const SCHEMA_V6: &str = r#"
UPDATE chunks
   SET note_id = (SELECT note_id FROM assets WHERE id = chunks.asset_id)
 WHERE kind = 'image' AND asset_id IS NOT NULL;
"#;

/// What a screenshot is, so two copies of one print can be recognised as one.
/// Filled in by the indexer: SQL cannot read a file, so the column starts empty
/// and the next scan hashes whatever is still missing.
const SCHEMA_V7: &str = r#"
ALTER TABLE assets ADD COLUMN content_hash TEXT;
CREATE INDEX assets_hash_idx ON assets(content_hash);
"#;

/// Passages are now cut at 1 200 characters (`parse::SPLIT_AT`) and a text
/// passage's vector carries its trail of headings (`embed::pending`). A note is
/// recut only when its hash changes, so the hashes are cleared for the next scan;
/// the old vectors do not look stale, so they are dropped, as in V5.
const SCHEMA_V8: &str = r#"
UPDATE notes SET content_hash = '';
DELETE FROM embeddings;
"#;

#[cfg(test)]
mod tests {
    use super::*;

    /// The repair for indexes written while the two columns could disagree.
    #[test]
    fn the_migration_gives_a_drifted_chunk_back_its_note() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            INSERT INTO notes (id, path, title, folder, mtime, content_hash) VALUES
              (1, 'react/hooks.md', 'Hooks', 'react', 0, 'a');
            INSERT INTO assets (id, note_id, path, ocr_json, mtime) VALUES
              (1, 1, 'react/_assets/print.png', NULL, 0);
            INSERT INTO chunks (note_id, asset_id, kind, heading_path, text) VALUES
              (NULL, 1, 'image', 'print.png', 'cache no print');
            "#,
        )
        .unwrap();

        // The statement that ships, rather than a faked downgrade: the ladder
        // only ever runs forwards, and re-entering it would try to add a column
        // that is already there.
        conn.execute_batch(SCHEMA_V6).unwrap();

        let note_id: Option<i64> = conn
            .query_row("SELECT note_id FROM chunks WHERE kind = 'image'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(note_id, Some(1));

        // The update trigger rewrites the row's entry in the external-content
        // index. Only `note_id` changed, so the text it deletes is the text that
        // was indexed — asserted rather than reasoned about, because an FTS that
        // disagrees with its content table fails silently, as an empty search.
        conn.execute_batch("INSERT INTO chunks_fts(chunks_fts) VALUES ('integrity-check')")
            .expect("o índice de busca não sobreviveu à migração");
        let found: i64 = conn
            .query_row("SELECT count(*) FROM chunks_fts WHERE chunks_fts MATCH 'cache'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(found, 1);
    }
}
