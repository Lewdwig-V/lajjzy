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

    /// Run a jj command and return a status message.
    ///
    /// jj prints human-readable feedback to stderr on success, so stderr is
    /// preferred when non-empty. stdout is returned as a fallback.
    fn run_jj(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("jj")
            .args(args)
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| format!("Failed to run `jj {}`", args.join(" ")))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if !output.status.success() {
            bail!("{}", if stderr.is_empty() { &stdout } else { &stderr });
        }

        Ok(if stderr.is_empty() { stdout } else { stderr })
    }
}

/// Truncate text to its first line, capped at 50 chars, for status bar display.
/// Uses `char_indices` to avoid panicking on multibyte UTF-8 (emoji, CJK).
fn first_line_preview(text: &str) -> String {
    let first = text.lines().next().unwrap_or("");
    if first.chars().count() > 50 {
        let truncate_at = first.char_indices().nth(47).map_or(first.len(), |(i, _)| i);
        format!("{}...", &first[..truncate_at])
    } else {
        first.to_string()
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
        if bytes[i] == b'|' || bytes[i] == b'-' || bytes[i] == b'@' || bytes[i] == b'~' {
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
        b'C' if after_status.starts_with(' ') => Some(crate::types::FileChange {
            path: after_status.trim().to_string(),
            status: crate::types::FileStatus::Conflicted,
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
fn parse_graph_output(output: &str, op_id: String) -> Result<GraphData> {
    let mut lines = Vec::new();
    let mut details = std::collections::HashMap::new();
    let mut working_copy_index = None;
    let mut current_change_id: Option<String> = None;

    for raw_line in output.lines() {
        if let Some(sep_pos) = raw_line.find(UNIT_SEP) {
            let display = &raw_line[..sep_pos];
            let metadata = &raw_line[sep_pos + UNIT_SEP.len_utf8()..];
            let fields: Vec<&str> = metadata.split(RECORD_SEP).collect();

            if fields.len() < 11 {
                bail!(
                    "Expected 11 metadata fields, got {}: {:?}",
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
                    conflict_count: usize::from(fields[8] == "true"),
                    files: vec![],
                    parents: if fields[10].is_empty() {
                        vec![]
                    } else {
                        fields[10].split(' ').map(String::from).collect()
                    },
                },
            );

            let glyph_end = display.find(|c: char| c.is_alphanumeric()).unwrap_or(0);
            let glyph_prefix = display[..glyph_end].to_string();

            lines.push(crate::types::GraphLine {
                raw: display.to_string(),
                change_id: Some(change_id),
                glyph_prefix,
            });
        } else if let Some(file_change) = parse_file_line(raw_line) {
            if let Some(last_id) = &current_change_id
                && let Some(detail) = details.get_mut(last_id)
            {
                detail.files.push(file_change);
            }
        } else {
            lines.push(crate::types::GraphLine {
                glyph_prefix: raw_line.to_string(),
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

    Ok(GraphData::new(lines, details, working_copy_index, op_id))
}

/// Parse git-format diff output into hunks.
///
/// Pre-`@@` header lines (`diff --git`, `index`, `new file mode`, etc.) are
/// collected. If the diff has no `@@` hunks (chmod-only, binary, pure rename),
/// a single hunk with these header lines is returned so the user sees something
/// rather than "(empty diff)".
#[expect(clippy::unnecessary_wraps)] // Result kept for forward-compatibility with error paths
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

/// Flush the accumulated state for the current file into `files`.
///
/// Handles the header-only case (chmod, binary, pure rename) by creating a
/// synthetic hunk so the caller always sees at least one hunk per file.
fn flush_file_diff(
    files: &mut Vec<crate::types::FileDiff>,
    path: Option<String>,
    hunks: &mut Vec<crate::types::DiffHunk>,
    open_hunk: &mut Option<crate::types::DiffHunk>,
    header_lines: &mut Vec<crate::types::DiffLine>,
) {
    let Some(p) = path else { return };
    if let Some(h) = open_hunk.take() {
        hunks.push(h);
    }
    if hunks.is_empty() && !header_lines.is_empty() {
        hunks.push(crate::types::DiffHunk {
            header: String::new(),
            lines: std::mem::take(header_lines),
        });
    }
    header_lines.clear();
    files.push(crate::types::FileDiff {
        path: p,
        hunks: std::mem::take(hunks),
    });
}

/// Parse git-format diff output for an entire change, grouping hunks by file.
///
/// Splits on `diff --git a/<path> b/<path>` lines. Each file gets its own
/// [`FileDiff`][crate::types::FileDiff]. Header-only diffs (chmod, binary,
/// pure rename) produce a synthetic hunk so callers always see something.
#[expect(clippy::unnecessary_wraps)] // Result kept for forward-compatibility with error paths
fn parse_file_diffs(output: &str) -> Result<Vec<crate::types::FileDiff>> {
    let mut files: Vec<crate::types::FileDiff> = Vec::new();

    // State for the file currently being accumulated.
    let mut current_path: Option<String> = None;
    let mut current_hunks: Vec<crate::types::DiffHunk> = Vec::new();
    let mut current_hunk: Option<crate::types::DiffHunk> = None;
    let mut header_lines: Vec<crate::types::DiffLine> = Vec::new();

    for line in output.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            // Flush the previous file before starting a new one.
            flush_file_diff(
                &mut files,
                current_path.take(),
                &mut current_hunks,
                &mut current_hunk,
                &mut header_lines,
            );

            // Extract path from `a/<path> b/<path>` — take the part after ` b/`.
            let path = rest
                .find(" b/")
                .map_or_else(|| rest.to_string(), |i| rest[i + 3..].to_string());
            current_path = Some(path);
        } else if line.starts_with("@@") {
            if let Some(h) = current_hunk.take() {
                current_hunks.push(h);
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
        } else if current_path.is_some() {
            // Pre-@@ header lines (index, new file mode, --- +++, etc.)
            header_lines.push(crate::types::DiffLine {
                kind: crate::types::DiffLineKind::Header,
                content: line.to_string(),
            });
        }
    }

    // Flush the last file.
    flush_file_diff(
        &mut files,
        current_path.take(),
        &mut current_hunks,
        &mut current_hunk,
        &mut header_lines,
    );

    Ok(files)
}

/// Parse the raw output of `jj op log` with our custom template into a list of `OpLogEntry`.
///
/// Expected line format: `<id>\x1f<description>\x1e<timestamp>`
fn parse_op_log_output(output: &str) -> Result<Vec<crate::types::OpLogEntry>> {
    let mut entries = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(sep_pos) = line.find(UNIT_SEP) {
            let id = &line[..sep_pos];
            let rest = &line[sep_pos + UNIT_SEP.len_utf8()..];
            let fields: Vec<&str> = rest.split(RECORD_SEP).collect();
            if fields.len() < 2 {
                bail!(
                    "Expected at least 2 op log fields after id, got {}: {:?}",
                    fields.len(),
                    fields
                );
            }
            entries.push(crate::types::OpLogEntry {
                id: id.to_string(),
                description: fields[0].to_string(),
                timestamp: fields[1].to_string(),
            });
        }
    }

    if entries.is_empty() && output.lines().any(|l| !l.trim().is_empty()) {
        bail!(
            "Parsed op log output but found zero entries. \
             The jj op log template output format may have changed."
        );
    }

    Ok(entries)
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

    fn change_diff(&self, change_id: &str) -> Result<Vec<crate::types::FileDiff>> {
        let output = Command::new("jj")
            .args(["diff", "-r", change_id, "--git", "--color=never"])
            .current_dir(&self.workspace_root)
            .output()
            .with_context(|| format!("Failed to run `jj diff` for {change_id}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj diff failed for {change_id}: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj diff output was not valid UTF-8")?;

        parse_file_diffs(&stdout)
    }

    fn load_graph(&self, revset: Option<&str>) -> Result<GraphData> {
        // Capture current operation head for snapshot versioning.
        let op_id = self
            .run_jj(&[
                "op",
                "log",
                "--limit=1",
                "--no-graph",
                "-T",
                "self.id().short(16)",
            ])
            .unwrap_or_else(|_| "unknown".to_string());

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
            " ++ \"\\x1e\" ++ parents.map(|p| p.change_id().short()).join(\" \")",
            " ++ \"\\n\"",
        );

        let mut args = vec!["log", "--summary", "--color=never", "-T", template];
        if let Some(rev) = revset {
            args.extend(["-r", rev]);
        }
        let output = Command::new("jj")
            .args(&args)
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run `jj log`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj log failed: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj log output was not valid UTF-8")?;

        parse_graph_output(&stdout, op_id)
    }

    fn op_log(&self) -> Result<Vec<crate::types::OpLogEntry>> {
        // Template produces: <id>\x1f<description>\x1e<timestamp>\n
        // The \x1f and \x1e are jj template escape sequences inside quoted strings.
        let template = concat!(
            "self.id().short(8)",
            " ++ \"\\x1f\" ++ description",
            " ++ \"\\x1e\" ++ self.time().start().ago()",
            " ++ \"\\n\"",
        );

        let output = Command::new("jj")
            .args(["op", "log", "--no-graph", "--color=never", "-T", template])
            .current_dir(&self.workspace_root)
            .output()
            .context("Failed to run `jj op log`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("jj op log failed: {}", stderr.trim());
        }

        let stdout =
            String::from_utf8(output.stdout).context("jj op log output was not valid UTF-8")?;

        parse_op_log_output(&stdout)
    }

    fn describe(&self, change_id: &str, text: &str) -> Result<String> {
        self.run_jj(&["describe", change_id, "-m", text])?;
        let preview = first_line_preview(text);
        Ok(format!("Described {change_id}: \"{preview}\""))
    }

    fn new_change(&self, after: &str) -> Result<String> {
        // --insert-after rebases children onto the new change, inserting into
        // the stack rather than forking it. Plain `jj new <rev>` would create
        // a sibling branch on non-leaf changes.
        self.run_jj(&["new", "--insert-after", after])?;
        Ok(format!("Created new change after {after}"))
    }

    fn edit_change(&self, change_id: &str) -> Result<String> {
        self.run_jj(&["edit", change_id])?;
        Ok(format!("Now editing {change_id}"))
    }

    fn abandon(&self, change_id: &str) -> Result<String> {
        self.run_jj(&["abandon", change_id])?;
        Ok(format!("Abandoned {change_id}"))
    }

    fn undo(&self) -> Result<String> {
        self.run_jj(&["undo"])?;
        Ok("Undid last operation".into())
    }

    fn redo(&self) -> Result<String> {
        self.run_jj(&["redo"])?;
        Ok("Redid last operation".into())
    }

    fn bookmark_set(&self, change_id: &str, name: &str) -> Result<String> {
        self.run_jj(&["bookmark", "set", name, "-r", change_id])?;
        Ok(format!("Set bookmark \"{name}\" on {change_id}"))
    }

    fn bookmark_delete(&self, name: &str) -> Result<String> {
        self.run_jj(&["bookmark", "delete", name])?;
        Ok(format!("Deleted bookmark \"{name}\""))
    }

    fn git_push(&self, bookmark: &str) -> Result<String> {
        self.run_jj(&["git", "push", "--bookmark", bookmark])?;
        Ok(format!("Pushed {bookmark}"))
    }

    fn git_fetch(&self) -> Result<String> {
        self.run_jj(&["git", "fetch"])?;
        Ok("Fetched from remote".into())
    }

    fn rebase_single(&self, source: &str, destination: &str) -> Result<String> {
        self.run_jj(&["rebase", "-r", source, "--onto", destination])?;
        Ok(format!("Rebased {source} onto {destination}"))
    }

    fn rebase_with_descendants(&self, source: &str, destination: &str) -> Result<String> {
        self.run_jj(&["rebase", "-s", source, "--onto", destination])?;
        Ok(format!("Rebased {source} + descendants onto {destination}"))
    }

    fn split(
        &self,
        change_id: &str,
        selections: &[crate::types::FileHunkSelection],
    ) -> Result<String> {
        // Our convention: "selected = moves to child".
        // `jj split <paths>` keeps <paths> in the ORIGINAL commit; the rest go
        // to the NEW child.  So we must pass the *complement* — the files that
        // are NOT fully selected — to jj split, keeping them in the original.
        // Fully-selected files (selected_hunks.len() == total_hunks) are the
        // ones we want in the child, so they must NOT appear in the args.
        let all_paths: std::collections::HashSet<&str> =
            selections.iter().map(|s| s.path.as_str()).collect();
        let fully_selected: std::collections::HashSet<&str> = selections
            .iter()
            .filter(|s| !s.selected_hunks.is_empty() && s.selected_hunks.len() == s.total_hunks)
            .map(|s| s.path.as_str())
            .collect();
        let mut keep_in_original: Vec<&str> =
            all_paths.difference(&fully_selected).copied().collect();
        if keep_in_original.is_empty() {
            bail!("Cannot split: all files are fully selected (nothing would remain in original)");
        }
        // Sort for determinism in tests and CLI output.
        keep_in_original.sort_unstable();
        // `-m ""` prevents jj from opening $EDITOR for the first commit's
        // description.  The second (child) commit keeps the original description.
        let mut args = vec!["split", "-r", change_id, "-m", "", "--"];
        args.extend(keep_in_original);
        self.run_jj(&args)?;
        Ok(format!("Split {change_id}"))
    }

    fn squash_partial(
        &self,
        change_id: &str,
        selections: &[crate::types::FileHunkSelection],
    ) -> Result<String> {
        let mut selected_paths: Vec<&str> = selections
            .iter()
            .filter(|s| !s.selected_hunks.is_empty())
            .map(|s| s.path.as_str())
            .collect();
        if selected_paths.is_empty() {
            bail!("No files selected for squash");
        }
        // Sort for determinism.
        selected_paths.sort_unstable();
        // `-u` / `--use-destination-message` prevents jj from opening $EDITOR
        // for a combined description when both source and destination have
        // non-empty descriptions.
        let mut args = vec!["squash", "-r", change_id, "-u", "--"];
        args.extend(selected_paths);
        self.run_jj(&args)?;
        Ok(format!("Squashed from {change_id}"))
    }

    fn conflict_sides(&self, _change_id: &str, _path: &str) -> Result<crate::types::ConflictData> {
        bail!("conflict_sides not yet implemented — pending jj-lib integration")
    }

    fn resolve_file(&self, change_id: &str, path: &str, content: Vec<u8>) -> Result<String> {
        // Defense-in-depth: verify change_id is the working copy
        let wc_id = self.run_jj(&["log", "-r", "@", "--no-graph", "-T", "change_id.short()"])?;
        if wc_id.trim() != change_id {
            bail!(
                "resolve_file: change {} is not the working copy (@={})",
                change_id,
                wc_id.trim()
            );
        }
        let abs_path = self.workspace_root.join(path);
        std::fs::write(&abs_path, &content)
            .with_context(|| format!("Failed to write resolved file: {}", abs_path.display()))?;
        Ok(format!("Resolved {path}"))
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
◉  abc12 alice 2m ago\x1Fabc12\x1Eaaa11\x1Ealice\x1Ealice@ex.com\x1E2m ago\x1Efix bug\x1Emain\x1Efalse\x1Efalse\x1E@\x1E
│  fix bug
◉  def45 bob 1h ago\x1Fdef45\x1Ebbb22\x1Ebob\x1Ebob@ex.com\x1E1h ago\x1Eadd feature\x1E\x1Efalse\x1Efalse\x1E\x1E
│  add feature";

        let graph = parse_graph_output(output, String::new()).unwrap();
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
◉  abc12 alice 2m ago\x1Fabc12\x1Eaaa11\x1Ealice\x1Ea@b.c\x1E2m\x1Edesc\x1E\x1Efalse\x1Efalse\x1E@\x1E
│  some description
│";

        let graph = parse_graph_output(output, String::new()).unwrap();
        assert!(graph.lines[0].change_id.is_some());
        assert!(graph.lines[1].change_id.is_none());
        assert!(graph.lines[2].change_id.is_none());
    }

    #[test]
    fn parse_graph_output_empty_bookmarks() {
        let output = "◉  x y 1m\x1Fx\x1Ey\x1Ez\x1Ea@b\x1E1m\x1Ed\x1E\x1Efalse\x1Efalse\x1E\x1E";
        let graph = parse_graph_output(output, String::new()).unwrap();
        assert!(graph.details.get("x").unwrap().bookmarks.is_empty());
    }

    #[test]
    fn parse_graph_output_rejects_incomplete_metadata() {
        let output = "◉  x y 1m\x1Fx\x1Ey"; // only 2 fields
        let result = parse_graph_output(output, String::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Expected 11"));
    }

    #[test]
    fn parse_graph_output_empty_input() {
        let graph = parse_graph_output("", String::new()).unwrap();
        assert!(graph.lines.is_empty());
        assert!(graph.node_indices().is_empty());
    }

    #[test]
    fn parse_graph_output_with_file_summary() {
        let output = "\
@  mpvponzr add bar\x1Fmpvponzr\x1Edbd5259e\x1ELewdwig\x1Etest@test.com\x1E1m ago\x1Eadd bar\x1E\x1Efalse\x1Efalse\x1E@\x1E
│  A bar.txt
│  M foo.txt
○  mrvmvrsz add foo\x1Fmrvmvrsz\x1Ecbfd5aa0\x1ELewdwig\x1Etest@test.com\x1E2m ago\x1Eadd foo\x1E\x1Efalse\x1Efalse\x1E\x1E
│  A foo.txt
◆  zzzzzzzz (no description)\x1Fzzzzzzzz\x1E000000000000\x1E\x1E\x1E56y ago\x1E\x1E\x1Etrue\x1Efalse\x1E\x1E";

        let graph = parse_graph_output(output, String::new()).unwrap();

        // File lines are compacted out of graph.lines; only node lines remain.
        assert_eq!(graph.lines.len(), 3);

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
@  abc rename\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1Erename\x1E\x1Efalse\x1Efalse\x1E@\x1E
│  R {foo.txt => bar.txt}";

        let graph = parse_graph_output(output, String::new()).unwrap();
        let detail = graph.details.get("abc").unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Renamed);
        assert!(detail.files[0].path.contains("=>"));
    }

    #[test]
    fn parse_graph_output_file_after_tilde_glyph() {
        // jj uses `~` to indicate elided revisions; file lines after it must
        // still be compacted into the detail, not leak into graph lines.
        let output = "\
◆  zuk root\x1Fzuk\x1E000\x1ELewdwig\x1Ea@b\x1E8h ago\x1E\x1E\x1Efalse\x1Efalse\x1E\x1E
~  A LICENSE";

        let graph = parse_graph_output(output, String::new()).unwrap();
        // Only the node line — the file line is compacted out.
        assert_eq!(graph.lines.len(), 1);
        let detail = graph.details.get("zuk").unwrap();
        assert_eq!(detail.files.len(), 1);
        assert_eq!(detail.files[0].path, "LICENSE");
        assert_eq!(detail.files[0].status, crate::types::FileStatus::Added);
    }

    #[test]
    fn parse_graph_output_no_files_for_empty_change() {
        let output = "\
@  abc (no description)\x1Fabc\x1E111\x1Ea\x1Ea@b\x1E1m\x1E\x1E\x1Etrue\x1Efalse\x1E@\x1E";

        let graph = parse_graph_output(output, String::new()).unwrap();
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
        let graph = backend.load_graph(None).unwrap();
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
    fn parse_op_log_output_basic() {
        let output = "abc12345\x1fcreate bookmark main\x1e2 hours ago\ndef67890\x1fsnapshot working copy\x1e3 hours ago\n";
        let entries = parse_op_log_output(output).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "abc12345");
        assert!(entries[0].description.contains("bookmark"));
    }

    #[test]
    fn parse_op_log_output_rejects_lines_without_separator() {
        let output = "some output without separator\nanother line\n";
        let result = parse_op_log_output(output);
        assert!(result.is_err());
    }

    #[test]
    fn parse_op_log_output_empty() {
        let entries = parse_op_log_output("").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn op_log_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        Command::new("jj")
            .args(["git", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let entries = backend.op_log().unwrap();
        assert!(!entries.is_empty());
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
        let graph = backend.load_graph(None).unwrap();

        assert!(!graph.node_indices().is_empty());
        assert!(graph.working_copy_index.is_some());
        for &idx in graph.node_indices() {
            assert!(graph.detail_at(idx).is_some());
        }
    }

    // ── mutation method tests ────────────────────────────────────────────────

    fn init_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        Command::new("jj")
            .args(["git", "init"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        tmp
    }

    #[test]
    fn abandon_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Create a second change so we can abandon it without touching the root.
        backend.describe("@", "to-abandon").unwrap();
        backend.new_change("@").unwrap();

        let graph_before = backend.load_graph(None).unwrap();
        let node_count_before = graph_before.node_indices().len();

        // Identify the non-working-copy, non-root change to abandon.
        let wc_idx = graph_before.working_copy_index.unwrap();
        let wc_id = graph_before.lines[wc_idx]
            .change_id
            .as_ref()
            .unwrap()
            .clone();
        // The parent of the working copy is the one we described.
        let parent_id = graph_before
            .node_indices()
            .iter()
            .filter_map(|&i| graph_before.lines[i].change_id.as_ref())
            .find(|id| *id != &wc_id)
            .and_then(|id| {
                let d = graph_before.details.get(id)?;
                if d.description == "to-abandon" {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .expect("should find the 'to-abandon' change");

        backend.abandon(&parent_id).unwrap();

        let graph_after = backend.load_graph(None).unwrap();
        assert!(
            graph_after.node_indices().len() < node_count_before,
            "node count should decrease after abandon"
        );
        assert!(
            !graph_after.details.contains_key(&parent_id),
            "abandoned change should not appear in graph"
        );
    }

    #[test]
    fn describe_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        backend.describe("@", "my description").unwrap();

        let graph = backend.load_graph(None).unwrap();
        let wc_idx = graph.working_copy_index.unwrap();
        let change_id = graph.lines[wc_idx].change_id.as_ref().unwrap();
        let detail = graph.details.get(change_id).unwrap();
        assert_eq!(detail.description, "my description");
    }

    #[test]
    fn new_change_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        let graph_before = backend.load_graph(None).unwrap();
        let node_count_before = graph_before.node_indices().len();

        backend.new_change("@").unwrap();

        let graph_after = backend.load_graph(None).unwrap();
        assert!(
            graph_after.node_indices().len() > node_count_before,
            "node count should increase after new_change"
        );
    }

    #[test]
    fn new_change_inserts_into_stack() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Create a stack: root -> parent -> child
        Command::new("jj")
            .args(["describe", "-m", "parent"])
            .current_dir(tmp.path())
            .status()
            .unwrap();
        Command::new("jj")
            .args(["new", "-m", "child"])
            .current_dir(tmp.path())
            .status()
            .unwrap();

        // Get the parent's change ID
        let graph = backend.load_graph(None).unwrap();
        let parent_id = graph
            .node_indices()
            .iter()
            .find_map(|&i| {
                let cid = graph.lines[i].change_id.as_ref()?;
                let detail = graph.details.get(cid)?;
                if detail.description == "parent" {
                    Some(cid.clone())
                } else {
                    None
                }
            })
            .expect("parent change not found");

        // Insert after parent — should NOT fork, child should be reparented
        backend.new_change(&parent_id).unwrap();

        let graph_after = backend.load_graph(None).unwrap();
        // The new change is now between parent and child (inserted, not forked).
        // Verify child still exists (wasn't lost) and there's one more node.
        let has_child = graph_after.node_indices().iter().any(|&i| {
            graph_after
                .details
                .get(
                    graph_after.lines[i]
                        .change_id
                        .as_ref()
                        .unwrap_or(&String::new()),
                )
                .is_some_and(|d| d.description == "child")
        });
        assert!(
            has_child,
            "child change should still exist after insert-after"
        );
    }

    #[test]
    fn edit_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Create two changes: wc (@) and a parent with a known description.
        backend.describe("@", "first").unwrap();
        backend.new_change("@").unwrap();
        backend.describe("@", "second").unwrap();

        let graph_before = backend.load_graph(None).unwrap();
        let prev_wc_idx = graph_before.working_copy_index.unwrap();
        let prev_wc_id = graph_before.lines[prev_wc_idx]
            .change_id
            .as_ref()
            .unwrap()
            .clone();

        // Find the "first" change.
        let first_id = graph_before
            .node_indices()
            .iter()
            .filter_map(|&i| graph_before.lines[i].change_id.as_ref())
            .find(|id| {
                graph_before
                    .details
                    .get(*id)
                    .is_some_and(|d| d.description == "first")
            })
            .expect("should find 'first' change")
            .clone();

        backend.edit_change(&first_id).unwrap();

        let graph_after = backend.load_graph(None).unwrap();
        let new_wc_idx = graph_after.working_copy_index.unwrap();
        let new_wc_id = graph_after.lines[new_wc_idx].change_id.as_ref().unwrap();

        assert_ne!(new_wc_id, &prev_wc_id, "working copy should have moved");
        assert_eq!(new_wc_id, &first_id, "working copy should now be 'first'");
    }

    #[test]
    fn undo_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        backend.describe("@", "before undo").unwrap();
        {
            let graph = backend.load_graph(None).unwrap();
            let wc_idx = graph.working_copy_index.unwrap();
            let id = graph.lines[wc_idx].change_id.as_ref().unwrap();
            assert_eq!(graph.details[id].description, "before undo");
        }

        backend.undo().unwrap();

        let graph = backend.load_graph(None).unwrap();
        let wc_idx = graph.working_copy_index.unwrap();
        let id = graph.lines[wc_idx].change_id.as_ref().unwrap();
        assert_ne!(
            graph.details[id].description, "before undo",
            "description should be reverted after undo"
        );
    }

    #[test]
    fn redo_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        backend.describe("@", "redo target").unwrap();
        backend.undo().unwrap();

        {
            let graph = backend.load_graph(None).unwrap();
            let wc_idx = graph.working_copy_index.unwrap();
            let id = graph.lines[wc_idx].change_id.as_ref().unwrap();
            assert_ne!(graph.details[id].description, "redo target");
        }

        backend.redo().unwrap();

        let graph = backend.load_graph(None).unwrap();
        let wc_idx = graph.working_copy_index.unwrap();
        let id = graph.lines[wc_idx].change_id.as_ref().unwrap();
        assert_eq!(
            graph.details[id].description, "redo target",
            "description should be restored after redo"
        );
    }

    #[test]
    fn bookmark_set_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        backend.bookmark_set("@", "mybookmark").unwrap();

        let graph = backend.load_graph(None).unwrap();
        let has_bookmark = graph.details.values().any(|d| {
            d.bookmarks
                .iter()
                .any(|b| b.trim_end_matches('*') == "mybookmark")
        });
        assert!(has_bookmark, "mybookmark should appear in graph details");
    }

    #[test]
    fn bookmark_delete_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        backend.bookmark_set("@", "todelete").unwrap();
        backend.bookmark_delete("todelete").unwrap();

        let graph = backend.load_graph(None).unwrap();
        let has_bookmark = graph.details.values().any(|d| {
            d.bookmarks
                .iter()
                .any(|b| b.trim_end_matches('*') == "todelete")
        });
        assert!(!has_bookmark, "todelete should be gone from graph details");
    }

    /// Push requires a configured remote — skip in CI unless a remote is present.
    #[test]
    #[ignore = "requires a configured git remote"]
    fn git_push_requires_remote() {
        // This test is intentionally ignored because it needs a real git remote.
        // To run manually: cargo test -p lajjzy-core git_push_requires_remote -- --ignored
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        backend.bookmark_set("@", "main").unwrap();
        let result = backend.git_push("main");
        // With no remote configured this will error; the test just validates the
        // method compiles and returns a Result.
        let _ = result;
    }

    /// Fetch requires a configured remote — skip in CI unless a remote is present.
    #[test]
    #[ignore = "requires a configured git remote"]
    fn git_fetch_requires_remote() {
        // This test is intentionally ignored because it needs a real git remote.
        // To run manually: cargo test -p lajjzy-core git_fetch_requires_remote -- --ignored
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let result = backend.git_fetch();
        // With no remote configured this will error; the test just validates the
        // method compiles and returns a Result.
        let _ = result;
    }

    #[test]
    fn load_graph_with_revset_filters_results() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        backend.describe("@", "keep me").unwrap();
        backend.new_change("@").unwrap();
        backend.describe("@", "also keep").unwrap();

        let full = backend.load_graph(None).unwrap();
        let full_count = full.node_indices().len();

        let filtered = backend.load_graph(Some("@")).unwrap();
        assert!(filtered.node_indices().len() < full_count);
        assert!(filtered.working_copy_index.is_some());
    }

    #[test]
    fn load_graph_with_invalid_revset_returns_error() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        let result = backend.load_graph(Some("not_a_valid_revset!!!"));
        assert!(result.is_err());
    }

    #[test]
    fn load_graph_includes_parent_ids() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        backend.describe("@", "parent").unwrap();
        backend.new_change("@").unwrap();
        backend.describe("@", "child").unwrap();

        let graph = backend.load_graph(None).unwrap();
        let child_detail = graph
            .node_indices()
            .iter()
            .find_map(|&i| {
                let cid = graph.lines[i].change_id.as_ref()?;
                let detail = graph.details.get(cid)?;
                if detail.description == "child" {
                    Some(detail)
                } else {
                    None
                }
            })
            .expect("child change not found");
        assert!(
            !child_detail.parents.is_empty(),
            "child should have parent IDs"
        );
    }

    /// Build a 4-node stack: root → A → B → C.
    /// Returns `(backend, root_id, a_id, b_id, c_id)`.
    fn build_stack(tmp: &tempfile::TempDir) -> (JjCliBackend, String, String, String, String) {
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Initial working copy is root-level — label it A.
        backend.describe("@", "commit-A").unwrap();
        backend.new_change("@").unwrap();
        backend.describe("@", "commit-B").unwrap();
        backend.new_change("@").unwrap();
        backend.describe("@", "commit-C").unwrap();

        let graph = backend.load_graph(None).unwrap();

        let find_id = |desc: &str| -> String {
            graph
                .node_indices()
                .iter()
                .find_map(|&i| {
                    let cid = graph.lines[i].change_id.as_ref()?;
                    let detail = graph.details.get(cid)?;
                    if detail.description == desc {
                        Some(cid.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| panic!("change '{desc}' not found in graph"))
        };

        let a_id = find_id("commit-A");
        let b_id = find_id("commit-B");
        let c_id = find_id("commit-C");

        // Identify root: it has an empty description and no parents.
        let root_id = graph
            .node_indices()
            .iter()
            .find_map(|&i| {
                let change_id = graph.lines[i].change_id.as_ref()?;
                let detail = graph.details.get(change_id)?;
                if detail.description.is_empty() {
                    Some(change_id.clone())
                } else {
                    None
                }
            })
            .expect("root change not found");

        (backend, root_id, a_id, b_id, c_id)
    }

    #[test]
    fn rebase_single_moves_b_leaves_c_under_a() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let (backend, root_id, a_id, b_id, c_id) = build_stack(&tmp);

        // Rebase B onto root: B should become a child of root.
        // C (descendant of B) is reparented onto A (B's old parent).
        backend.rebase_single(&b_id, &root_id).unwrap();

        let graph = backend.load_graph(None).unwrap();

        let detail = |id: &str| -> &crate::types::ChangeDetail {
            graph
                .details
                .get(id)
                .unwrap_or_else(|| panic!("{id} not in graph"))
        };

        // B's parent should now be root.
        assert!(
            detail(&b_id).parents.contains(&root_id),
            "after rebase_single, B should have root as parent; parents={:?}",
            detail(&b_id).parents
        );

        // C should be reparented onto A (descendants are spliced).
        assert!(
            detail(&c_id).parents.contains(&a_id),
            "after rebase_single, C should have A as parent; parents={:?}",
            detail(&c_id).parents
        );
    }

    #[test]
    fn rebase_with_descendants_moves_b_and_c_under_root() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let (backend, root_id, _a_id, b_id, c_id) = build_stack(&tmp);

        // Rebase B + descendants onto root: both B and C should move under root.
        backend.rebase_with_descendants(&b_id, &root_id).unwrap();

        let graph = backend.load_graph(None).unwrap();

        let detail = |id: &str| -> &crate::types::ChangeDetail {
            graph
                .details
                .get(id)
                .unwrap_or_else(|| panic!("{id} not in graph"))
        };

        // B's parent should be root.
        assert!(
            detail(&b_id).parents.contains(&root_id),
            "after rebase_with_descendants, B should have root as parent; parents={:?}",
            detail(&b_id).parents
        );

        // C's parent should be B (preserved relative order).
        assert!(
            detail(&c_id).parents.contains(&b_id),
            "after rebase_with_descendants, C should still be a child of B; parents={:?}",
            detail(&c_id).parents
        );
    }

    // ── parse_file_diffs tests ───────────────────────────────────────────────

    #[test]
    fn parse_file_diffs_empty() {
        let files = parse_file_diffs("").unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn parse_file_diffs_multi_file() {
        let output = "\
diff --git a/foo.txt b/foo.txt
new file mode 100644
--- /dev/null
+++ b/foo.txt
@@ -0,0 +1,1 @@
+hello
diff --git a/bar.txt b/bar.txt
new file mode 100644
--- /dev/null
+++ b/bar.txt
@@ -0,0 +1,1 @@
+world";
        let files = parse_file_diffs(output).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "foo.txt");
        assert_eq!(files[1].path, "bar.txt");
        assert!(!files[0].hunks.is_empty());
        assert!(!files[1].hunks.is_empty());
    }

    #[test]
    fn parse_file_diffs_single_file_multi_hunk() {
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
        let files = parse_file_diffs(output).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "foo.txt");
        assert_eq!(files[0].hunks.len(), 2);
    }

    #[test]
    fn parse_file_diffs_header_only_file() {
        // chmod-only diff: has headers but no @@ hunks — should produce a synthetic hunk.
        let output = "\
diff --git a/script.sh b/script.sh
old mode 100644
new mode 100755";
        let files = parse_file_diffs(output).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "script.sh");
        assert_eq!(files[0].hunks.len(), 1);
        assert!(files[0].hunks[0].header.is_empty());
        assert_eq!(
            files[0].hunks[0].lines[0].kind,
            crate::types::DiffLineKind::Header
        );
    }

    #[test]
    fn parse_file_diffs_path_with_spaces() {
        // Paths containing spaces: the ` b/` marker split must still work.
        let output = "\
diff --git a/my file.txt b/my file.txt
--- a/my file.txt
+++ b/my file.txt
@@ -1 +1 @@
-old
+new";
        let files = parse_file_diffs(output).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "my file.txt");
    }

    #[test]
    fn change_diff_returns_grouped_file_diffs() {
        if !jj_available() {
            eprintln!("Skipping");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("foo.txt"), "hello\n").unwrap();
        std::fs::write(tmp.path().join("bar.txt"), "world\n").unwrap();
        backend.describe("@", "add files").unwrap();

        let files = backend.change_diff("@").unwrap();
        assert!(files.len() >= 2);
        assert!(files.iter().any(|f| f.path == "foo.txt"));
        assert!(files.iter().any(|f| f.path == "bar.txt"));
        for f in &files {
            assert!(!f.hunks.is_empty(), "file {} should have hunks", f.path);
        }
    }

    // ── split / squash_partial tests ─────────────────────────────────────────

    #[test]
    fn split_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        std::fs::write(tmp.path().join("keep.txt"), "keep\n").unwrap();
        std::fs::write(tmp.path().join("move.txt"), "move\n").unwrap();
        backend.describe("@", "two files").unwrap();

        // "move.txt" is fully selected — it should end up in the child change.
        // "keep.txt" is NOT selected — it stays in the original.
        let selections = vec![
            crate::types::FileHunkSelection {
                path: "keep.txt".into(),
                selected_hunks: vec![],
                total_hunks: 1,
            },
            crate::types::FileHunkSelection {
                path: "move.txt".into(),
                selected_hunks: vec![0],
                total_hunks: 1,
            },
        ];
        backend.split("@", &selections).unwrap();

        let graph = backend.load_graph(None).unwrap();
        // root + original (keep.txt) + child (move.txt) = at least 3 node lines.
        assert!(
            graph.node_indices().len() >= 3,
            "expected at least 3 change nodes after split, got {}",
            graph.node_indices().len()
        );
    }

    #[test]
    fn split_rejects_all_files_selected() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "a\n").unwrap();
        backend.describe("@", "one file").unwrap();

        let selections = vec![crate::types::FileHunkSelection {
            path: "a.txt".into(),
            selected_hunks: vec![0],
            total_hunks: 1,
        }];
        let result = backend.split("@", &selections);
        assert!(
            result.is_err(),
            "split with all files selected should return an error"
        );
    }

    #[test]
    fn squash_partial_on_real_repo() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();

        // Set up: parent change, then a child change with a file.
        backend.describe("@", "parent").unwrap();
        backend.new_change("@").unwrap();
        std::fs::write(tmp.path().join("file.txt"), "content\n").unwrap();
        backend.describe("@", "child with file").unwrap();

        let selections = vec![crate::types::FileHunkSelection {
            path: "file.txt".into(),
            selected_hunks: vec![0],
            total_hunks: 1,
        }];
        backend.squash_partial("@", &selections).unwrap();

        // After squash_partial the child may be abandoned (it became empty).
        // file.txt should now appear in the parent.
        let graph = backend.load_graph(None).unwrap();
        assert!(
            graph.node_indices().len() >= 2,
            "expected at least 2 node lines after squash_partial"
        );
        let file_in_parent = graph
            .node_indices()
            .iter()
            .filter_map(|&i| graph.detail_at(i))
            .any(|d| d.files.iter().any(|f| f.path == "file.txt"));
        assert!(
            file_in_parent,
            "file.txt should appear in a change after squash_partial"
        );
    }

    #[test]
    fn squash_partial_rejects_empty_selection() {
        if !jj_available() {
            eprintln!("Skipping: jj not in PATH");
            return;
        }
        let tmp = init_repo();
        let backend = JjCliBackend::new(tmp.path()).unwrap();
        std::fs::write(tmp.path().join("a.txt"), "a\n").unwrap();
        backend.describe("@", "has file").unwrap();

        // No hunks selected for any file.
        let selections = vec![crate::types::FileHunkSelection {
            path: "a.txt".into(),
            selected_hunks: vec![],
            total_hunks: 1,
        }];
        let result = backend.squash_partial("@", &selections);
        assert!(
            result.is_err(),
            "squash_partial with no selected hunks should return an error"
        );
    }
}
