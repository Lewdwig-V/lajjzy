use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::forge::{ForgeBackend, ForgeKind, PrInfo, PrState, ReviewStatus};

pub struct GhCliForge {
    workspace_root: PathBuf,
    available: bool,
}

impl GhCliForge {
    pub fn new(workspace_root: &Path) -> Self {
        let available = Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        Self {
            workspace_root: workspace_root.to_path_buf(),
            available,
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl ForgeBackend for GhCliForge {
    fn forge_kind(&self) -> Option<ForgeKind> {
        if self.available {
            Some(ForgeKind::GitHub)
        } else {
            None
        }
    }

    fn fetch_status(&self) -> Result<Option<Vec<PrInfo>>> {
        if !self.available {
            return Ok(None);
        }

        let output = Command::new("gh")
            .args([
                "pr",
                "list",
                "--state",
                "open",
                "--limit",
                "100",
                "--json",
                "number,title,state,headRefName,reviewDecision,url",
            ])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run gh pr list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh pr list failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8(output.stdout).context("gh output was not valid UTF-8")?;

        let prs = parse_gh_pr_list(&stdout)?;
        Ok(Some(prs))
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrJson {
    number: u32,
    title: String,
    state: String,
    head_ref_name: String,
    review_decision: String,
    #[serde(default)]
    url: String,
}

fn parse_gh_pr_list(json: &str) -> Result<Vec<PrInfo>> {
    let raw: Vec<GhPrJson> =
        serde_json::from_str(json).context("Failed to parse gh pr list JSON")?;

    Ok(raw
        .into_iter()
        .map(|pr| PrInfo {
            number: pr.number,
            title: pr.title,
            state: match pr.state.as_str() {
                "MERGED" => PrState::Merged,
                "CLOSED" => PrState::Closed,
                _ => PrState::Open, // "OPEN" or unknown → Open
            },
            review: match pr.review_decision.as_str() {
                "APPROVED" => ReviewStatus::Approved,
                "CHANGES_REQUESTED" => ReviewStatus::ChangesRequested,
                "REVIEW_REQUIRED" => ReviewStatus::ReviewRequired,
                _ => ReviewStatus::Unknown,
            },
            head_ref: pr.head_ref_name,
            url: pr.url,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_open_pr_approved() {
        let json = r#"[{
            "number": 42,
            "title": "feat: add feature",
            "state": "OPEN",
            "headRefName": "feature-x",
            "reviewDecision": "APPROVED",
            "url": "https://github.com/owner/repo/pull/42"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 42);
        assert_eq!(prs[0].state, PrState::Open);
        assert_eq!(prs[0].review, ReviewStatus::Approved);
        assert_eq!(prs[0].head_ref, "feature-x");
        assert_eq!(prs[0].url, "https://github.com/owner/repo/pull/42");
    }

    #[test]
    fn parse_changes_requested() {
        let json = r#"[{
            "number": 15,
            "title": "fix: bug",
            "state": "OPEN",
            "headRefName": "fix-bug",
            "reviewDecision": "CHANGES_REQUESTED",
            "url": "https://github.com/owner/repo/pull/15"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].review, ReviewStatus::ChangesRequested);
    }

    #[test]
    fn parse_empty_review_decision() {
        let json = r#"[{
            "number": 1,
            "title": "test",
            "state": "OPEN",
            "headRefName": "test",
            "reviewDecision": "",
            "url": ""
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].review, ReviewStatus::Unknown);
    }

    #[test]
    fn parse_empty_list() {
        let prs = parse_gh_pr_list("[]").unwrap();
        assert!(prs.is_empty());
    }

    #[test]
    fn parse_merged_pr() {
        let json = r#"[{
            "number": 10,
            "title": "merged",
            "state": "MERGED",
            "headRefName": "old",
            "reviewDecision": "APPROVED",
            "url": "https://github.com/owner/repo/pull/10"
        }]"#;
        let prs = parse_gh_pr_list(json).unwrap();
        assert_eq!(prs[0].state, PrState::Merged);
    }

    #[test]
    fn parse_malformed_json_returns_error() {
        assert!(parse_gh_pr_list("not json").is_err());
    }
}
