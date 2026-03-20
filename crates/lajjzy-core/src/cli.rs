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

/// Strip leading graph glyphs from a line to get the content.
fn strip_graph_glyphs(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            i += 1;
            continue;
        }
        if bytes[i] == b'|' || bytes[i] == b'-' || bytes[i] == b'@' {
            i += 1;
            continue;
        }
        if i + 2 < bytes.len() && bytes[i] == 0xE2 {
            i += 3;
            continue;
        }
        break;
    }
    &line[i..]
}

/// Try to parse a continuation line as a file change summary.
fn parse_file_line(raw_line: &str) -> Option<crate::types::FileChange> {
    let content = strip_graph_glyphs(raw_line);
    if content.len() < 2 {
        return None;
    }

    let first_byte = content.as_bytes()[0];
    if !first_byte.is_ascii() {
        return None;
    }

    let after_status = &content[1..];

    match first_byte {
        b'A' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Added,
        }),
        b'M' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Modified,
        }),
        b'D' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Deleted,
        }),
        b'R' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Renamed,
        }),
        c if c.is_ascii_uppercase() && after_status.starts_with(' ') => {
            Some(crate::types::FileChange {
                path: after_status.trim().to_string(),
                status: crate::types::FileStatus::Unknown(c as char),
            })
        }
        _ => None,
    }
}

/// Parse the raw output of `jj log` with our custom template into `GraphData`.
fn parse_graph_output(output: &str) -> Result<GraphData> {
    let mut lines = Vec::new();
    let mut details = std::collections::HashMap::new();
    let mut working_copy_index = None;
    let mut current_change_id: Option<String> = None;

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
            current_change_id = Some(change_id.clone());
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
        } else if let Some(file_change) = parse_file_line(raw_line) {
            if let Some(last_id) = &current_change_id
                && let Some(detail) = details.get_mut(last_id)
            {
                detail.files.push(file_change);
            }
            lines.push(crate::types::GraphLine {
                raw: raw_line.to_string(),
                change_id: None,
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

/// Parse git-format diff output into hunks.
///
/// Pre-`@@` header lines (`diff --git`, `index`, `new file mode`, etc.) are
/// collected. If the diff has no `@@` hunks (chmod-only, binary, pure rename),
/// a single hunk with these header lines is returned so the user sees something
/// rather than "(empty diff)".
#[allow(clippy::unnecessary_wraps)] // Result kept for forward-compatibility with error paths
fn parse_diff_output(output: &str) -> Result<Vec<crate::types::DiffHunk>> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<crate::types::DiffHunk> = None;
    let mut header_lines: Vec<crate::types::DiffLine> = Vec::new();

    for line in output.lines() {
        if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            current_hunk = Some(crate::types::DiffHunk {
                header: line.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(ref mut hunk) = current_hunk {
            let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
                (crate::types::DiffLineKind::Added, rest)
            } else if let Some(rest) = line.strip_prefix('-') {
                (crate::types::DiffLineKind::Removed, rest)
            } else if let Some(rest) = line.strip_prefix(' ') {
                (crate::types::DiffLineKind::Context, rest)
            } else {
                (crate::types::DiffLineKind::Context, line)
            };
            hunk.lines.push(crate::types::DiffLine {
                kind,
                content: content.to_string(),
            });
        } else {
            // Pre-@@ header lines (diff --git, index, new file mode, etc.)
            header_lines.push(crate::types::DiffLine {
                kind: crate::types::DiffLineKind::Header,
                content: line.to_string(),
            });
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    // If no @@ hunks but we have header lines (chmod-only, binary, pure rename),
    // create a synthetic hunk so the user sees the header info.
    if hunks.is_empty() && !header_lines.is_empty() {
        hunks.push(crate::types::DiffHunk {
            header: String::new(),
            lines: header_lines,
        });
    }

    Ok(hunks)
}

impl RepoBackend for JjCliBackend {
    fn file_diff(&self, change_id: &str, path: &str) -> Result<Vec<crate::types::DiffHunk>> {
        let output = Command::new("jj")
            .args(["diff", "-r", change_id, "--git", "--color=never", path])
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| format!("Failed to run `jj diff` for {path}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj diff failed for {path}: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj diff output was not valid UTF-8")?;

        parse_diff_output(&stdout)
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
            " ++ \"\\n\"",
        );

        let output = Command::new("jj")
            .args(["log", "--summary", "--color=never", "-T", template])
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
    fn parse_graph_output_with_file_summary() {
        let output = "\
@  mpvponzr add bar\x1Fmpvponzr\x1Edbd5259e\x1ELewdwig\x1Etest@test.com\x1E1m ago\x1Eadd bar\x1E\x1Efalse\x1Efalse\x1E@
│  A bar.txt
│  M foo.txt
○  mrvmvrsz add foo\x1Fmrvmvrsz\x1Ecbfd5aa0\x1ELewdwig\x1Etest@test.com\x1E2m ago\x1Eadd foo\x1E\x1Efalse\x1Efalse\x1E
│  A foo.txt
◆  zzzzzzzz (no description)\x1Fzzzzzzzz\x1E000000000000\x1E\x1E\x1E56y ago\x1E\x1E\x1Etrue\x1Efalse\x1E";

        let graph = parse_graph_output(output).unwrap();

        let detail = graph.details.get("mpvponzr").unwrap();
        assert_eq!(detail.files.len(), 2);
        assert_eq!(detail.files[0].path, "bar.txt");
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Added);
        assert_eq!(detail.files[1].path, "foo.txt");
        assert_eq!(detail.files[1].status, crate::types::FileStatus::Modified);

        let detail2 = graph.details.get("mrvmvrsz").unwrap();
        assert_eq!(detail2.files.len(), 1);
        assert_eq!(detail2.files[0].path, "foo.txt");

        let detail3 = graph.details.get("zzzzzzzz").unwrap();
        assert!(detail3.files.is_empty());
    }

    #[test]
    fn parse_graph_output_rename() {
        let output = "\
@  abc rename\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1Erename\x1E\x1Efalse\x1Efalse\x1E@
│  R {foo.txt => bar.txt}";

        let graph = parse_graph_output(output).unwrap();
        let detail = graph.details.get("abc").unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Renamed);
        assert!(detail.files[0].path.contains("=>"));
    }

    #[test]
    fn parse_graph_output_no_files_for_empty_change() {
        let output = "\
@  abc (no description)\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1E\x1E\x1Etrue\x1Efalse\x1E@";

        let graph = parse_graph_output(output).unwrap();
        let detail = graph.details.get("abc").unwrap();
        assert!(detail.files.is_empty());
    }

    #[test]
    fn parse_diff_output_single_hunk() {
        let output = "\
diff --git a/foo.txt b/foo.txt
index ce01362..2e09960 100644
--- a/foo.txt
+++ b/foo.txt
@@ -1,1 +1,1 @@
-hello
+modified";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].header.contains("-1,1 +1,1"));
        assert_eq!(hunks[0].lines.len(), 2);
        assert_eq!(hunks[0].lines[0].kind, crate::types::DiffLineKind::Removed);
        assert_eq!(hunks[0].lines[0].content, "hello");
        assert_eq!(hunks[0].lines[1].kind, crate::types::DiffLineKind::Added);
        assert_eq!(hunks[0].lines[1].content, "modified");
    }

    #[test]
    fn parse_diff_output_new_file() {
        let output = "\
diff --git a/bar.txt b/bar.txt
new file mode 100644
index 0000000..cc628cc
--- /dev/null
+++ b/bar.txt
@@ -0,0 +1,1 @@
+world";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].lines.len(), 1);
        assert_eq!(hunks[0].lines[0].kind, crate::types::DiffLineKind::Added);
    }

    #[test]
    fn parse_diff_output_multi_hunk() {
        let output = "\
diff --git a/foo.txt b/foo.txt
--- a/foo.txt
+++ b/foo.txt
@@ -1,3 +1,3 @@
 line1
-old2
+new2
 line3
@@ -10,3 +10,3 @@
 line10
-old11
+new11
 line12";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].lines.len(), 4); // context, removed, added, context
        assert_eq!(hunks[1].lines.len(), 4); // context, removed, added, context
    }

    #[test]
    fn parse_diff_output_empty() {
        let hunks = parse_diff_output("").unwrap();
        assert!(hunks.is_empty());
    }

    #[test]
    fn parse_diff_output_header_only() {
        // chmod-only or binary diffs have headers but no @@ hunks
        let output = "\
diff --git a/script.sh b/script.sh
old mode 100644
new mode 100755";

        let hunks = parse_diff_output(output).unwrap();
        assert_eq!(hunks.len(), 1);
        assert!(hunks[0].header.is_empty()); // synthetic header
        assert_eq!(hunks[0].lines.len(), 3);
        assert_eq!(hunks[0].lines[0].kind, crate::types::DiffLineKind::Header);
        assert!(hunks[0].lines[0].content.contains("diff --git"));
    }

    #[test]
    fn file_diff_on_real_repo() {
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
        std::fs::write(tmp.path().join("test.txt"), "hello\n").unwrap();
        Command::new("jj")
            .args(["describe", "-m", "add test"])
            .current_dir(tmp.path())
            .status()
            .unwrap();

        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let graph = backend.load_graph().unwrap();
        let wc_idx = graph.working_copy_index.unwrap();
        let change_id = graph.lines[wc_idx].change_id.as_ref().unwrap();

        let hunks = backend.file_diff(change_id, "test.txt").unwrap();
        assert!(!hunks.is_empty());
        assert!(
            hunks[0]
                .lines
                .iter()
                .any(|l| l.kind == crate::types::DiffLineKind::Added)
        );
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
