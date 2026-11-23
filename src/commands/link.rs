use crate::commands::{Context, resolve_project};
use crate::config::load_token;
use crate::sanitize::sanitize_terminal_text;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: LinkFile =
        serde_json::from_str(&raw).with_context(|| format!("invalid JSON in {}", path.display()))?;
    let slug = parsed.project_slug.trim().to_string();
    if slug.is_empty() {
        return Ok(None);
    }
    Ok(Some(slug))
}

pub fn run(ctx: &Context, slug: &str) -> Result<()> {
    let clean_slug = slug.trim();
    if clean_slug.is_empty() {
        bail!("project slug cannot be empty");
    }

    let token = load_token().context("connect your account first with 'weoc connect'")?;
    let project = resolve_project(ctx, &token.access_token, Some(clean_slug))?;

    let link = LinkFile {
        project_slug: project.slug.clone(),
        project_id: Some(project.id.clone()),
    };
    let path = link_file_path()?;
    let content = serde_json::to_string_pretty(&link)?;
    fs::write(&path, format!("{content}\n"))
        .with_context(|| format!("failed to write {}", path.display()))?;

    println!(
        "Linked directory to project '{}' ({}) in {}",
        sanitize_terminal_text(&project.slug),
        sanitize_terminal_text(&project.name),
        LINK_FILE_NAME
    );
    Ok(())
}
