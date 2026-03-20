use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::backend::RepoBackend;
use crate::types::GraphData;

/// Backend that shells out to the `jj` CLI.
pub struct JjCliBackend {
    /// Absolute path to the workspace root (from `jj root`).
    workspace_root: PathBuf,
}

impl JjCliBackend {
    /// Create a new backend for the given directory.
    /// Validates that `jj` is installed and the path is inside a jj workspace.
    pub fn new(path: &Path) -> Result<Self> {
        let output = Command::new("jj")
            .arg("root")
            .current_dir(path)
            .output()
            .context("Failed to run `jj root`. Is jj installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("Not a jj workspace: {}", stderr.trim());
        }

        let root = String::from_utf8(output.stdout)
            .context("jj root output was not valid UTF-8")?
            .trim()
            .to_string();

        Ok(Self {
            workspace_root: PathBuf::from(root),
        })
    }

    /// Returns the workspace root path.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

/// Separator between display text and metadata in template output.
const UNIT_SEP: char = '\x1F';
/// Separator between fields within metadata.
const RECORD_SEP: char = '\x1E';

/// Parse the raw output of `jj log` with our custom template into `GraphData`.
fn parse_graph_output(output: &str) -> Result<GraphData> {
    let mut lines = Vec::new();
    let mut details = std::collections::HashMap::new();
    let mut working_copy_index = None;

    for raw_line in output.lines() {
        if let Some(sep_pos) = raw_line.find(UNIT_SEP) {
            let display = &raw_line[..sep_pos];
            let metadata = &raw_line[sep_pos + UNIT_SEP.len_utf8()..];
            let fields: Vec<&str> = metadata.split(RECORD_SEP).collect();

            if fields.len() < 10 {
                bail!(
                    "Expected 10 metadata fields, got {}: {:?}",
                    fields.len(),
                    fields
                );
            }

            let change_id = fields[0].to_string();
            let is_working_copy = !fields[9].is_empty();

            let index = lines.len();
            if is_working_copy {
                working_copy_index = Some(index);
            }

            if details.contains_key(&change_id) {
                bail!(
                    "Duplicate short change ID '{change_id}'. \
                     This may indicate a truncation collision."
                );
            }
            details.insert(
                change_id.clone(),
                crate::types::ChangeDetail {
                    commit_id: fields[1].to_string(),
                    author: fields[2].to_string(),
                    email: fields[3].to_string(),
                    timestamp: fields[4].to_string(),
                    description: fields[5].to_string(),
                    bookmarks: if fields[6].is_empty() {
                        vec![]
                    } else {
                        fields[6].split(' ').map(String::from).collect()
                    },
                    is_empty: fields[7] == "true",
                    has_conflict: fields[8] == "true",
                    files: vec![],
                },
            );

            lines.push(crate::types::GraphLine {
                raw: display.to_string(),
                change_id: Some(change_id),
            });
        } else {
            lines.push(crate::types::GraphLine {
                raw: raw_line.to_string(),
                change_id: None,
            });
        }
    }

    if !output.trim().is_empty() && details.is_empty() {
        bail!(
            "Parsed {} lines of jj output but found zero change nodes. \
             The jj template output format may have changed.",
            lines.len()
        );
    }

    Ok(GraphData::new(lines, details, working_copy_index))
}

impl RepoBackend for JjCliBackend {
    fn file_diff(&self, _change_id: &str, _path: &str) -> Result<Vec<crate::types::DiffHunk>> {
        todo!("Implemented in Task 5")
    }

    fn load_graph(&self) -> Result<GraphData> {
        // `working_copies` is empty in jj 0.39.0; use self.current_working_copy() instead.
        let template = concat!(
            "change_id.short() ++ \" \" ++ ",
            "coalesce(author.name(), \"anonymous\") ++ \" \" ++ ",
            "committer.timestamp().ago()",
            " ++ \"\\x1f\"",
            " ++ change_id.short()",
            " ++ \"\\x1e\" ++ commit_id.short()",
            " ++ \"\\x1e\" ++ coalesce(author.name(), \"\")",
            " ++ \"\\x1e\" ++ coalesce(author.email(), \"\")",
            " ++ \"\\x1e\" ++ committer.timestamp().ago()",
            " ++ \"\\x1e\" ++ coalesce(description.first_line(), \"\")",
            " ++ \"\\x1e\" ++ bookmarks",
            " ++ \"\\x1e\" ++ empty",
            " ++ \"\\x1e\" ++ conflict",
            " ++ \"\\x1e\" ++ if(self.current_working_copy(), \"@\", \"\")",
            " ++ \"\\n\" ++ coalesce(description.first_line(), \"(no description)\")",
        );

        let output = Command::new("jj")
            .args(["log", "--color=never", "-T", template])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run `jj log`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj log failed: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj log output was not valid UTF-8")?;

        parse_graph_output(&stdout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn jj_available() -> bool {
        Command::new("jj").arg("--version").output().is_ok()
    }

    #[test]
    fn new_fails_on_non_workspace() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let result = JjCliBackend::new(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn new_succeeds_on_jj_workspace() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let status = Command::new("jj")
            .args(["git", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        assert!(status.success());

        let backend = JjCliBackend::new(tmp.path()).unwrap();
        assert_eq!(backend.workspace_root(), tmp.path().canonicalize().unwrap());
    }

    #[test]
    fn parse_graph_output_basic() {
        let output = "\
◉  abc12 alice 2m ago\x1Fabc12\x1Eaaa11\x1Ealice\x1Ealice@ex.com\x1E2m ago\x1Efix bug\x1Emain\x1Efalse\x1Efalse\x1E@
│  fix bug
◉  def45 bob 1h ago\x1Fdef45\x1Ebbb22\x1Ebob\x1Ebob@ex.com\x1E1h ago\x1Eadd feature\x1E\x1Efalse\x1Efalse\x1E
│  add feature";

        let graph = parse_graph_output(output).unwrap();
        assert_eq!(graph.lines.len(), 4);
        assert_eq!(graph.node_indices(), vec![0, 2]);
        assert_eq!(graph.working_copy_index, Some(0));

        let detail = graph.details.get("abc12").unwrap();
        assert_eq!(detail.author, "alice");
        assert_eq!(detail.bookmarks, vec!["main"]);

        let detail2 = graph.details.get("def45").unwrap();
        assert!(detail2.bookmarks.is_empty());
    }

    #[test]
    fn parse_graph_output_connector_lines_have_no_change_id() {
        let output = "\
◉  abc12 alice 2m ago\x1Fabc12\x1Eaaa11\x1Ealice\x1Ea@b.c\x1E2m\x1Edesc\x1E\x1Efalse\x1Efalse\x1E@
│  some description
│";

        let graph = parse_graph_output(output).unwrap();
        assert!(graph.lines[0].change_id.is_some());
        assert!(graph.lines[1].change_id.is_none());
        assert!(graph.lines[2].change_id.is_none());
    }

    #[test]
    fn parse_graph_output_empty_bookmarks() {
        let output = "◉  x y 1m\x1Fx\x1Ey\x1Ez\x1Ea@b\x1E1m\x1Ed\x1E\x1Efalse\x1Efalse\x1E";
        let graph = parse_graph_output(output).unwrap();
        assert!(graph.details.get("x").unwrap().bookmarks.is_empty());
    }

    #[test]
    fn parse_graph_output_rejects_incomplete_metadata() {
        let output = "◉  x y 1m\x1Fx\x1Ey"; // only 2 fields
        let result = parse_graph_output(output);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Expected 10"));
    }

    #[test]
    fn parse_graph_output_empty_input() {
        let graph = parse_graph_output("").unwrap();
        assert!(graph.lines.is_empty());
        assert!(graph.node_indices().is_empty());
    }

    #[test]
    fn load_graph_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        Command::new("jj")
            .args(["git", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        Command::new("jj")
            .args(["describe", "-m", "test change"])
            .current_dir(tmp.path())
            .status()
            .unwrap();

        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let graph = backend.load_graph().unwrap();

        assert!(!graph.node_indices().is_empty());
        assert!(graph.working_copy_index.is_some());
        for &idx in graph.node_indices() {
            assert!(graph.detail_at(idx).is_some());
        }
    }
}
