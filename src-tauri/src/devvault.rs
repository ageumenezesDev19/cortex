//! The part of the vault coding agents write: `dev/`.
//!
//! A profile and a process folder shared by every project, and one folder per
//! project. This is the only code that writes the format, so the MCP server,
//! the command line and the app cannot drift into three dialects of it.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

use crate::vault;

pub const DEV: &str = "dev";
pub const PROJECTS: &str = "dev/projetos";
pub const MAP: &str = "dev/projetos/_mapa.md";
/// Evaluation reports quote their questions word for word; searched by
/// default, they become the best answer to every one of them.
pub const EVALS: &str = "dev/avaliacoes";

/// Shared by every project. Everything else belongs to exactly one.
const GLOBAL_KINDS: [&str; 2] = ["perfil", "processo"];
const PROJECT_KINDS: [&str; 3] = ["fato", "decisao", "armadilha"];

const MAP_HEADER: &str = "# Mapa de projetos

Liga a pasta de um repositório ao projeto em `dev/projetos/`. Uma pasta nova entra
sozinha com o próprio nome; corrija o nome aqui ou com a ferramenta `map_project`.

| pasta | projeto |
|---|---|
";

/// Where a kind of note lives, or why it cannot be written.
pub fn folder_for(kind: &str, project: Option<&str>) -> Result<String> {
    if GLOBAL_KINDS.contains(&kind) {
        if project.is_some() {
            bail!("`{kind}` vale para todos os projetos: grave sem `project`");
        }
        return Ok(format!("{DEV}/{kind}"));
    }
    if PROJECT_KINDS.contains(&kind) {
        let project =
            project.ok_or_else(|| anyhow!("`{kind}` é de um projeto: falta `project`"))?;
        return Ok(format!("{PROJECTS}/{project}"));
    }
    bail!(
        "tipo desconhecido: {kind}. Use um de: {}",
        GLOBAL_KINDS
            .iter()
            .chain(PROJECT_KINDS.iter())
            .copied()
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn is_project_kind(kind: &str) -> bool {
    PROJECT_KINDS.contains(&kind)
}

/// The `type` Claude Code's memory uses for each kind.
pub fn type_of(kind: &str) -> &'static str {
    match kind {
        "perfil" => "user",
        "processo" => "feedback",
        _ => "project",
    }
}

/// The first segment names the project; `ai-tax-desk/api` is a part of one.
pub fn project_of(name: &str) -> &str {
    name.split('/').next().unwrap_or(name)
}

pub fn valid_project(name: &str) -> bool {
    let parts: Vec<&str> = name.split('/').collect();
    parts.len() <= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

/// One row of the map: a repository folder and the project it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct Mapping {
    pub folder: PathBuf,
    pub project: String,
}

pub fn read_map(root: &Path) -> Vec<Mapping> {
    let Ok(source) = std::fs::read_to_string(root.join(MAP)) else {
        return Vec::new();
    };
    source.lines().filter_map(parse_row).collect()
}

fn parse_row(line: &str) -> Option<Mapping> {
    let cells: Vec<&str> = line
        .trim()
        .strip_prefix('|')?
        .strip_suffix('|')?
        .split('|')
        .map(str::trim)
        .collect();
    let [folder, project] = cells[..] else {
        return None;
    };
    if folder == "pasta" || folder.starts_with("---") || !valid_project(project) {
        return None;
    }
    Some(Mapping {
        folder: expand(folder),
        project: project.to_string(),
    })
}

fn expand(folder: &str) -> PathBuf {
    match (folder.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(folder),
    }
}

fn shown(folder: &Path) -> String {
    match dirs::home_dir().and_then(|home| folder.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => folder.display().to_string(),
    }
}

/// The repository a session runs in: the nearest folder holding `.git`.
pub fn repository_of(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|at| at.join(".git").exists())
        .map(Path::to_path_buf)
}

/// The project of the repository `dir` is in. A repository not in the map is
/// added under its own folder name, so nobody has to map anything by hand.
pub fn resolve_project(root: &Path, dir: &Path) -> Result<String> {
    let map = read_map(root);
    // A folder outside git is found only through the map.
    let Some(repository) = repository_of(dir) else {
        let row = map
            .iter()
            .filter(|row| dir.starts_with(&row.folder))
            .max_by_key(|row| row.folder.components().count())
            .ok_or_else(|| {
                anyhow!("a sessão não está num repositório git nem no mapa; passe `project`")
            })?;
        let _ = point_memory(root, &row.folder, &row.project);
        return Ok(row.project.clone());
    };
    if let Some(found) = map
        .iter()
        .filter(|row| repository.starts_with(&row.folder))
        .max_by_key(|row| row.folder.components().count())
    {
        // Every session start re-points the memory, so a deleted or edited local
        // settings file heals itself; a broken one must not hide the project.
        let _ = point_memory(root, &found.folder, &found.project);
        return Ok(found.project.clone());
    }

    let base = vault::slugify(&repository.file_name().unwrap_or_default().to_string_lossy());
    let base = if base.is_empty() {
        "projeto".to_string()
    } else {
        base
    };
    let mut name = base.clone();
    let mut n = 2;
    while map.iter().any(|row| row.project == name) {
        name = format!("{base}-{n}");
        n += 1;
    }
    set_mapping(root, &repository, &name)?;
    Ok(name)
}

/// Points a folder at a project, replacing whatever row it had.
pub fn set_mapping(root: &Path, folder: &Path, project: &str) -> Result<()> {
    if !valid_project(project) {
        bail!("nome de projeto inválido: {project}. Use minúsculas, números e hífen, com no máximo uma `/`");
    }
    let path = root.join(MAP);
    let source = std::fs::read_to_string(&path).unwrap_or_else(|_| MAP_HEADER.to_string());
    let mut lines: Vec<String> = source
        .lines()
        .filter(|line| parse_row(line).is_none_or(|row| row.folder != folder))
        .map(str::to_string)
        .collect();
    lines.push(format!("| {} | {project} |", shown(folder)));
    std::fs::create_dir_all(path.parent().context("mapa sem pasta")?)?;
    std::fs::write(&path, lines.join("\n") + "\n")?;
    point_memory(root, folder, project)
}

/// A project named outright must be one the vault knows, so a typo does not
/// open a second folder for the same project.
pub fn known_project(root: &Path, name: &str) -> Result<()> {
    let first = project_of(name);
    let mapped = read_map(root)
        .iter()
        .any(|row| project_of(&row.project) == first);
    if mapped || root.join(PROJECTS).join(first).is_dir() {
        return Ok(());
    }
    let mut names: Vec<String> = read_map(root).into_iter().map(|row| row.project).collect();
    names.sort();
    names.dedup();
    bail!(
        "projeto desconhecido: {name}. No mapa: {}. Para um novo, use `map_project`",
        names.join(", ")
    )
}

/// Claude Code's own memory format, so its native memory and the Cortex write
/// the same files: `name`, `description`, a `metadata` block, then the body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Note {
    pub name: String,
    pub description: String,
    pub metadata: Vec<(String, String)>,
    pub body: String,
}

impl Note {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.metadata
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    pub fn set_meta(&mut self, key: &str, value: &str) {
        match self.metadata.iter_mut().find(|(name, _)| name == key) {
            Some((_, old)) => *old = value.to_string(),
            None => self.metadata.push((key.to_string(), value.to_string())),
        }
    }

    pub fn render(&self) -> String {
        let mut out = format!(
            "---\nname: {}\ndescription: {}\nmetadata:\n",
            one_line(&self.name),
            quoted(&self.description)
        );
        for (key, value) in &self.metadata {
            out.push_str(&format!("  {key}: {}\n", one_line(value)));
        }
        out.push_str(&format!("---\n\n{}\n", self.body.trim()));
        out
    }

    /// Reads what Claude Code and this module write; anything else is `None`.
    pub fn parse(source: &str) -> Option<Note> {
        let rest = source.strip_prefix("---\n")?;
        let (head, body) = rest.split_once("\n---\n")?;
        let mut note = Note {
            body: body.trim().to_string(),
            ..Note::default()
        };
        for line in head.lines() {
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = unquoted(value.trim());
            if line.starts_with(' ') {
                note.metadata.push((key.trim().to_string(), value));
            } else if key == "name" {
                note.name = value;
            } else if key == "description" {
                note.description = value;
            }
        }
        (!note.name.is_empty()).then_some(note)
    }
}

/// A value that spans lines would end the front matter early.
fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A description holds colons and quotes; quoted, it stays one YAML string.
fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        one_line(value).replace('\\', "\\\\").replace('"', "\\\"")
    )
}

fn unquoted(value: &str) -> String {
    match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => inner.replace("\\\"", "\"").replace("\\\\", "\\"),
        None => value.to_string(),
    }
}

/// The index Claude Code loads at the start of a session, one line per memory.
pub const INDEX: &str = "MEMORY.md";

/// Puts the line for `file` in `folder`'s index, replacing the one it had.
pub fn index_put(root: &Path, folder: &str, file: &str, line: &str) -> Result<()> {
    let path = root.join(folder).join(INDEX);
    let mut lines = index_lines_without(&path, file);
    lines.push(line.to_string());
    std::fs::create_dir_all(path.parent().context("índice sem pasta")?)?;
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Takes `file` out of `folder`'s index and returns the line it had.
pub fn index_drop(root: &Path, folder: &str, file: &str) -> Result<Option<String>> {
    let path = root.join(folder).join(INDEX);
    let Ok(source) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let link = format!("]({file})");
    let old = source
        .lines()
        .find(|line| line.contains(&link))
        .map(str::to_string);
    if old.is_some() {
        std::fs::write(&path, index_lines_without(&path, file).join("\n") + "\n")?;
    }
    Ok(old)
}

fn index_lines_without(path: &Path, file: &str) -> Vec<String> {
    let link = format!("]({file})");
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.contains(&link) && !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

pub fn index_line(title: &str, file: &str) -> String {
    format!("- [{}]({file})", one_line(title))
}

/// Points Claude Code's own memory for `folder` at the project's folder in the
/// vault, through local settings that stay out of git.
pub fn point_memory(root: &Path, folder: &Path, project: &str) -> Result<()> {
    if !folder.is_dir() {
        return Ok(());
    }
    let settings = folder.join(".claude").join("settings.local.json");
    let mut json: serde_json::Value = match std::fs::read_to_string(&settings) {
        Ok(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("{} não é JSON", settings.display()))?,
        Err(_) => serde_json::json!({}),
    };
    let object = json
        .as_object_mut()
        .ok_or_else(|| anyhow!("{} não é um objeto", settings.display()))?;
    // Every part of a project (`desk/api`, `desk/web`) shares the project's memory.
    let target = root.join(PROJECTS).join(project_of(project)).display().to_string();
    if object.get("autoMemoryDirectory").and_then(|v| v.as_str()) != Some(target.as_str()) {
        object.insert("autoMemoryDirectory".into(), target.into());
        std::fs::create_dir_all(settings.parent().context("sem pasta .claude")?)?;
        std::fs::write(&settings, serde_json::to_string_pretty(&json)? + "\n")?;
    }
    // Written by hand, so git is told to leave it out; `.git` is a file in a worktree.
    let exclude = folder.join(".git").join("info").join("exclude");
    if folder.join(".git").is_dir() {
        let current = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !current
            .lines()
            .any(|line| line.trim() == ".claude/settings.local.json")
        {
            std::fs::create_dir_all(exclude.parent().context("sem .git/info")?)?;
            let sep = if current.is_empty() || current.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            std::fs::write(
                &exclude,
                format!("{current}{sep}.claude/settings.local.json\n"),
            )?;
        }
    }
    Ok(())
}

/// A file name for `title` in `folder` that nothing holds yet.
pub fn free_path(root: &Path, folder: &str, title: &str) -> String {
    let slug = vault::slugify(title);
    let slug = if slug.is_empty() {
        "nota".to_string()
    } else {
        slug
    };
    let mut candidate = format!("{folder}/{slug}.md");
    let mut n = 2;
    while root.join(&candidate).exists() {
        candidate = format!("{folder}/{slug}-{n}.md");
        n += 1;
    }
    candidate
}

/// Writes only inside `dev/`, and never into the evaluation reports: a recorded
/// run is not rewritten.
pub fn writable(root: &Path, relative: &str) -> Result<PathBuf> {
    let inside = |prefix: &str| relative == prefix || relative.starts_with(&format!("{prefix}/"));
    if !inside(DEV) {
        bail!("o Claude só escreve em {DEV}/: {relative}");
    }
    if inside(EVALS) {
        bail!("rodadas gravadas em {EVALS}/ não são reescritas: {relative}");
    }
    if !relative.ends_with(".md") {
        bail!("só notas .md: {relative}");
    }
    vault::resolve(root, relative)
}

pub fn today() -> String {
    // UTC days since the epoch to a civil date (Howard Hinnant's algorithm), so
    // a date needs no time zone crate.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn a_project_kind_needs_a_project_and_a_global_kind_refuses_one() {
        assert_eq!(
            folder_for("decisao", Some("cortex")).unwrap(),
            "dev/projetos/cortex"
        );
        assert!(folder_for("decisao", None).is_err());
        assert_eq!(folder_for("perfil", None).unwrap(), "dev/perfil");
        assert!(folder_for("perfil", Some("cortex")).is_err());
        assert!(folder_for("qualquer", None).is_err());
    }

    #[test]
    fn an_unmapped_repository_maps_itself_under_its_folder_name() {
        let vault = TempDir::new().unwrap();
        let work = TempDir::new().unwrap();
        let repo = work.path().join("Meu Projeto");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("src/deep")).unwrap();

        assert_eq!(
            resolve_project(vault.path(), &repo.join("src/deep")).unwrap(),
            "meu-projeto"
        );
        assert_eq!(
            read_map(vault.path()),
            vec![Mapping {
                folder: repo.clone(),
                project: "meu-projeto".into()
            }]
        );
        // Asked again, it is found rather than added twice.
        assert_eq!(resolve_project(vault.path(), &repo).unwrap(), "meu-projeto");
        assert_eq!(read_map(vault.path()).len(), 1);
    }

    #[test]
    fn two_repositories_can_share_a_project() {
        let vault = TempDir::new().unwrap();
        let work = TempDir::new().unwrap();
        for part in ["api", "web"] {
            let repo = work.path().join(format!("desk-{part}"));
            std::fs::create_dir_all(repo.join(".git")).unwrap();
            set_mapping(vault.path(), &repo, &format!("desk/{part}")).unwrap();
        }
        assert_eq!(
            resolve_project(vault.path(), &work.path().join("desk-web")).unwrap(),
            "desk/web"
        );
        assert!(known_project(vault.path(), "desk").is_ok());
        let settings = std::fs::read_to_string(work.path().join("desk-web/.claude/settings.local.json")).unwrap();
        assert!(settings.contains("dev/projetos/desk\""), "as partes dividem a memória do projeto: {settings}");
        assert!(known_project(vault.path(), "dsek").is_err());
    }

    #[test]
    fn remapping_a_folder_replaces_its_row() {
        let vault = TempDir::new().unwrap();
        let folder = Path::new("/tmp/algum-repo");
        set_mapping(vault.path(), folder, "um").unwrap();
        set_mapping(vault.path(), folder, "outro").unwrap();
        assert_eq!(
            read_map(vault.path()),
            vec![Mapping {
                folder: folder.into(),
                project: "outro".into()
            }]
        );
        assert!(set_mapping(vault.path(), folder, "Com Espaço").is_err());
    }

    #[test]
    fn a_note_survives_being_written_and_read_back() {
        let mut note = Note {
            name: "indice-esquema-8".into(),
            description: "Índice no esquema 8: o \"app\" converte".into(),
            metadata: vec![
                ("type".into(), "project".into()),
                ("source".into(), "duas\nlinhas".into()),
            ],
            body: "O app converte.\n\nO MCP recusa.".into(),
        };
        let read = Note::parse(&note.render()).unwrap();
        assert_eq!(
            read,
            Note {
                metadata: vec![
                    ("type".into(), "project".into()),
                    ("source".into(), "duas linhas".into())
                ],
                ..note.clone()
            }
        );

        note.set_meta("status", "obsolete");
        assert_eq!(
            Note::parse(&note.render()).unwrap().meta("status"),
            Some("obsolete")
        );
    }

    /// The file Claude Code itself wrote in the test of 2026-11-02.
    #[test]
    fn a_memory_written_by_claude_code_is_read() {
        let source = "---\nname: pnpm-not-npm\ndescription: Este projeto usa pnpm; nunca usar npm.\nmetadata:\n  node_type: memory\n  type: feedback\n  modified: 2026-11-02T11:13:43.463Z\n---\n\nNeste projeto, o gerenciador é o **pnpm**.\n";
        let note = Note::parse(source).unwrap();
        assert_eq!(note.name, "pnpm-not-npm");
        assert_eq!(note.meta("type"), Some("feedback"));
        assert_eq!(note.meta("modified"), Some("2026-11-02T11:13:43.463Z"));
    }

    #[test]
    fn the_index_keeps_one_line_per_memory() {
        let vault = TempDir::new().unwrap();
        index_put(vault.path(), "dev/processo", "a.md", "- [A](a.md)").unwrap();
        index_put(vault.path(), "dev/processo", "b.md", "- [B](b.md)").unwrap();
        index_put(vault.path(), "dev/processo", "a.md", "- [A de novo](a.md)").unwrap();
        assert_eq!(
            index_drop(vault.path(), "dev/processo", "b.md")
                .unwrap()
                .as_deref(),
            Some("- [B](b.md)")
        );
        let index = std::fs::read_to_string(vault.path().join("dev/processo/MEMORY.md")).unwrap();
        assert_eq!(index, "- [A de novo](a.md)\n");
    }

    #[test]
    fn mapping_a_repository_points_its_memory_at_the_vault_and_keeps_it_out_of_git() {
        let vault = TempDir::new().unwrap();
        let repo = TempDir::new().unwrap();
        std::fs::create_dir_all(repo.path().join(".git/info")).unwrap();
        std::fs::create_dir_all(repo.path().join(".claude")).unwrap();
        std::fs::write(
            repo.path().join(".claude/settings.local.json"),
            r#"{"permissions":{"allow":["Bash(ls)"]}}"#,
        )
        .unwrap();

        set_mapping(vault.path(), repo.path(), "alfa").unwrap();
        set_mapping(vault.path(), repo.path(), "alfa").unwrap();

        let settings: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo.path().join(".claude/settings.local.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            settings["permissions"]["allow"][0], "Bash(ls)",
            "o que já havia fica"
        );
        assert_eq!(
            settings["autoMemoryDirectory"],
            vault.path().join("dev/projetos/alfa").display().to_string()
        );
        let exclude = std::fs::read_to_string(repo.path().join(".git/info/exclude")).unwrap();
        assert_eq!(exclude.matches(".claude/settings.local.json").count(), 1);
    }

    #[test]
    fn a_mapped_folder_outside_git_is_found_through_the_map() {
        let vault = TempDir::new().unwrap();
        let folder = TempDir::new().unwrap();
        set_mapping(vault.path(), folder.path(), "estudo").unwrap();
        assert_eq!(
            resolve_project(vault.path(), &folder.path().join("aula")).unwrap(),
            "estudo"
        );
    }

    #[test]
    fn writes_stay_inside_dev_and_out_of_the_reports() {
        let root = Path::new("/vault");
        assert!(writable(root, "dev/processo/x.md").is_ok());
        assert!(writable(root, "ia/x.md").is_err());
        assert!(writable(root, "dev/../ia/x.md").is_err());
        assert!(writable(root, "dev/avaliacoes/rag/x.md").is_err());
        assert!(writable(root, "dev/processo/x.txt").is_err());
    }

    #[test]
    fn today_is_a_date() {
        let date = today();
        assert_eq!(date.len(), 10);
        assert!(date.starts_with("20"));
    }
}
