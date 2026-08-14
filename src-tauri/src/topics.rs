use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::vault;

/// The manifest as it sits on disk. The slug is the file name, so it is not
/// repeated inside.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
    /// Note **or** image paths, relative to the vault root. A manifest is used
    /// instead of frontmatter precisely because images have no frontmatter.
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub slug: String,
    pub name: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub members: Vec<String>,
}

pub fn dir(root: &Path) -> PathBuf {
    root.join(vault::TOPICS_DIR)
}

fn path_of(root: &Path, slug: &str) -> PathBuf {
    dir(root).join(format!("{slug}.json"))
}

pub fn load_all(root: &Path) -> Result<Vec<Topic>> {
    let dir = dir(root);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut topics: Vec<Topic> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().is_some_and(|e| e == "json"))
        .filter_map(|entry| read(&entry.path()).ok())
        .collect();

    topics.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(topics)
}

fn read(path: &Path) -> Result<Topic> {
    let slug = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| anyhow!("topic file without a name"))?;
    let manifest: Manifest = serde_json::from_str(&std::fs::read_to_string(path)?)?;

    Ok(Topic {
        slug,
        name: manifest.name,
        icon: manifest.icon,
        color: manifest.color,
        members: manifest.members,
    })
}

pub fn load(root: &Path, slug: &str) -> Result<Topic> {
    read(&path_of(root, slug))
}

pub fn save(root: &Path, topic: &Topic) -> Result<()> {
    std::fs::create_dir_all(dir(root))?;
    let manifest = Manifest {
        name: topic.name.clone(),
        icon: topic.icon.clone(),
        color: topic.color.clone(),
        members: topic.members.clone(),
    };
    std::fs::write(
        path_of(root, &topic.slug),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(())
}

pub fn delete(root: &Path, slug: &str) -> Result<()> {
    let path = path_of(root, slug);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// The database is a mirror, never the record. Topics are few, so replacing the
/// whole mirror is simpler than diffing and cannot drift.
pub fn sync_to_db(conn: &mut Connection, topics: &[Topic]) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM topic_members", [])?;
    tx.execute("DELETE FROM topics", [])?;
    {
        let mut insert_topic =
            tx.prepare("INSERT INTO topics (slug, name, icon, color) VALUES (?1, ?2, ?3, ?4)")?;
        let mut insert_member =
            tx.prepare("INSERT INTO topic_members (topic_slug, target_path) VALUES (?1, ?2)")?;

        for topic in topics {
            insert_topic.execute(rusqlite::params![
                topic.slug,
                topic.name,
                topic.icon,
                topic.color
            ])?;
            for member in &topic.members {
                insert_member.execute(rusqlite::params![topic.slug, member])?;
            }
        }
    }
    tx.commit()?;
    Ok(())
}

/// File names have to survive git, Finder and a URL one day, so accents are
/// folded rather than escaped.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for character in name.trim().to_lowercase().chars() {
        let folded = fold(character);
        if folded.is_alphanumeric() {
            slug.push(folded);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

fn fold(character: char) -> char {
    match character {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn folds_accents_and_collapses_separators() {
        assert_eq!(slugify("Programação   Funcional"), "programacao-funcional");
        assert_eq!(slugify("Next.js"), "next-js");
        assert_eq!(slugify("  React  "), "react");
    }

    #[test]
    fn never_produces_a_leading_or_trailing_dash() {
        assert_eq!(slugify("!!! C# !!!"), "c");
    }
}
