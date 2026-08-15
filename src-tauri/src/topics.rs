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
    /// Filled in by the app rather than by you. The panel keeps these apart so
    /// a dozen machine-made marks cannot bury the topics you curated.
    #[serde(default)]
    pub auto: bool,
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
    #[serde(default)]
    pub auto: bool,
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
        auto: manifest.auto,
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
        auto: topic.auto,
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

pub use crate::vault::slugify;

/// Files a path under a topic. Reads the manifest fresh every time, so adding a
/// member can never be the thing that drops the others.
/// Files a screenshot under a mark, creating the topic the first time that
/// mark is seen. Marked as `auto` so the panel can keep it out of your own list.
pub fn add_auto_member(root: &Path, slug: &str, name: &str, icon: Option<&str>, path: &str) -> Result<()> {
    if load(root, slug).is_err() {
        save(
            root,
            &Topic {
                slug: slug.to_string(),
                name: name.to_string(),
                icon: icon.map(str::to_string),
                color: None,
                auto: true,
                members: Vec::new(),
            },
        )?;
    }
    add_member(root, slug, path)
}

pub fn add_member(root: &Path, slug: &str, path: &str) -> Result<()> {
    let mut topic = load(root, slug)?;
    if topic.members.iter().any(|member| member == path) {
        return Ok(());
    }
    topic.members.push(path.to_string());
    save(root, &topic)
}

/// Keeps topic membership honest when a note is renamed or deleted. A manifest
/// pointing at a path that no longer exists rots in silence, which is the worst
/// kind of bug — the topic simply gets quieter over time.
///
/// `to` of `None` means the target is gone. Folder paths carry their children,
/// so a prefix match is part of the contract, not a convenience.
pub fn retarget(root: &Path, from: &str, to: Option<&str>) -> Result<()> {
    let prefix = format!("{from}/");

    for mut topic in load_all(root)? {
        let mut changed = false;
        let mut members = Vec::with_capacity(topic.members.len());

        for member in &topic.members {
            let rewritten = if member == from {
                to.map(str::to_string)
            } else if let Some(rest) = member.strip_prefix(&prefix) {
                to.map(|to| format!("{to}/{rest}"))
            } else {
                Some(member.clone())
            };

            match rewritten {
                Some(path) => {
                    changed |= &path != member;
                    members.push(path);
                }
                None => changed = true,
            }
        }

        if changed {
            topic.members = members;
            save(root, &topic)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault_with_topic(members: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        save(
            dir.path(),
            &Topic {
                slug: "react".into(),
                name: "React".into(),
                icon: None,
                color: None,
                auto: false,
                members: members.iter().map(|m| m.to_string()).collect(),
            },
        )
        .unwrap();
        dir
    }

    #[test]
    fn renaming_a_note_follows_it_into_the_manifest() {
        let dir = vault_with_topic(&["react/hooks.md", "next/actions.md"]);
        retarget(dir.path(), "react/hooks.md", Some("react/ganchos.md")).unwrap();

        let topic = load(dir.path(), "react").unwrap();
        assert_eq!(topic.members, ["react/ganchos.md", "next/actions.md"]);
    }

    #[test]
    fn renaming_a_folder_carries_its_children() {
        let dir = vault_with_topic(&["react/hooks.md", "react/_assets/print.png"]);
        retarget(dir.path(), "react", Some("reactjs")).unwrap();

        let topic = load(dir.path(), "react").unwrap();
        assert_eq!(topic.members, ["reactjs/hooks.md", "reactjs/_assets/print.png"]);
    }

    #[test]
    fn deleting_a_note_drops_it_from_the_manifest() {
        let dir = vault_with_topic(&["react/hooks.md", "next/actions.md"]);
        retarget(dir.path(), "react/hooks.md", None).unwrap();

        let topic = load(dir.path(), "react").unwrap();
        assert_eq!(topic.members, ["next/actions.md"]);
    }

    #[test]
    fn an_unrelated_rename_rewrites_nothing() {
        let dir = vault_with_topic(&["react/hooks.md"]);
        retarget(dir.path(), "outro/arquivo.md", Some("outro/novo.md")).unwrap();

        let topic = load(dir.path(), "react").unwrap();
        assert_eq!(topic.members, ["react/hooks.md"]);
    }

    #[test]
    fn adding_a_member_keeps_the_ones_already_there() {
        let dir = vault_with_topic(&["react/hooks.md"]);
        add_member(dir.path(), "react", "react/estado.md").unwrap();

        let topic = load(dir.path(), "react").unwrap();
        assert_eq!(topic.members, ["react/hooks.md", "react/estado.md"]);
    }

    #[test]
    fn adding_the_same_member_twice_does_not_duplicate_it() {
        let dir = vault_with_topic(&["react/hooks.md"]);
        add_member(dir.path(), "react", "react/hooks.md").unwrap();

        assert_eq!(load(dir.path(), "react").unwrap().members, ["react/hooks.md"]);
    }
}
