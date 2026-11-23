use crate::commands::{Context, resolve_project};
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const LINK_FILE_NAME: &str = ".weond.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkFile {
    project_slug: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<String>,
}

fn link_file_path() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("unable to determine current directory")?;
    Ok(cwd.join(LINK_FILE_NAME))
}

pub fn load_linked_slug() -> Result<Option<String>> {
    let path = link_file_path()?;
    load_linked_slug_from(&path)
}

pub fn load_linked_slug_from(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: LinkFile = serde_json::from_str(&raw)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    let slug = parsed.project_slug.trim().to_string();
    if slug.is_empty() {
        return Ok(None);
    }
    Ok(Some(slug))
}

pub fn save_link_file_at(path: &Path, project_slug: &str, project_id: &str) -> Result<()> {
    let link = LinkFile {
        project_slug: project_slug.to_string(),
        project_id: Some(project_id.to_string()),
    };
    let content = serde_json::to_string_pretty(&link)?;
    fs::write(path, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", path.display()))
}

pub fn run(ctx: &Context, slug: &str) -> Result<()> {
    let clean_slug = slug.trim();
    if clean_slug.is_empty() {
        bail!("project slug cannot be empty");
    }

    let auth = ctx.resolve_auth()?;
    let project = resolve_project(ctx, auth.access_token.as_str(), Some(clean_slug))?;

    let path = link_file_path()?;
    save_link_file_at(&path, &project.slug, &project.id)?;

    println!(
        "Linked directory to project '{}' ({}) in {}",
        sanitize_terminal_text(&project.slug),
        sanitize_terminal_text(&project.name),
        LINK_FILE_NAME
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_link_file() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join(LINK_FILE_NAME);
        assert_eq!(load_linked_slug_from(&path).unwrap(), None);

        save_link_file_at(&path, "my-app", "proj-123").unwrap();
        assert_eq!(
            load_linked_slug_from(&path).unwrap().as_deref(),
            Some("my-app")
        );
    }
}
