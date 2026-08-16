//! Git-based version control for tune files
//!
//! Provides local git integration for tracking tune changes over time,
//! allowing users to view history, compare versions, and restore previous tunes.

use git2::{
    BranchType, Commit, DiffOptions, Error as GitError, IndexAddOption, Repository, Signature,
    StatusOptions,
};
use std::path::Path;

const NOTE_PREFIX: &str = "LT-Note:";

/// Information about a commit in the tune history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    /// Short SHA (first 7 characters)
    pub sha_short: String,
    /// Full SHA hash
    pub sha: String,
    /// Commit message
    pub message: String,
    /// Optional user annotation
    pub annotation: Option<String>,
    /// Author name
    pub author: String,
    /// Commit timestamp (ISO 8601)
    pub timestamp: String,
    /// Whether this is the current HEAD
    pub is_head: bool,
}

pub fn format_commit_message(message: &str, annotation: Option<&str>) -> String {
    let note = annotation.map(str::trim).filter(|value| !value.is_empty());

    if let Some(note) = note {
        format!("{message}\n\n{NOTE_PREFIX} {note}")
    } else {
        message.to_string()
    }
}

fn extract_annotation(message: &str) -> Option<String> {
    message
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix(NOTE_PREFIX)
                .map(|rest| rest.trim().to_string())
        })
        .filter(|value| !value.is_empty())
}

/// Information about a branch
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BranchInfo {
    /// Branch name
    pub name: String,
    /// Whether this is the current branch
    pub is_current: bool,
    /// SHA of the branch tip
    pub tip_sha: String,
}

/// A change between two commits
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TuneChange {
    /// Name of the changed constant
    pub name: String,
    /// Value in the older commit (None if added)
    pub old_value: Option<String>,
    /// Value in the newer commit (None if deleted)
    pub new_value: Option<String>,
    /// Type of change: "added", "modified", "deleted"
    pub change_type: String,
}

/// Result of comparing two commits
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitDiff {
    /// SHA of the older commit
    pub from_sha: String,
    /// SHA of the newer commit
    pub to_sha: String,
    /// List of changes
    pub changes: Vec<TuneChange>,
    /// Files that changed
    pub files_changed: Vec<String>,
}

/// Version control operations for a project
pub struct VersionControl {
    repo: Repository,
}

impl VersionControl {
    /// Initialize a new git repository in the project folder
    pub fn init(project_path: &Path) -> Result<Self, GitError> {
        let repo = Repository::init(project_path)?;

        // Create initial .gitignore
        let gitignore_path = project_path.join(".gitignore");
        if !gitignore_path.exists() {
            let gitignore_content = r#"# LibreTune project gitignore
# Ignore temporary files
*.tmp
*.bak

# Ignore data logs (can be large)
datalogs/

# Ignore cached data
.cache/
"#;
            std::fs::write(&gitignore_path, gitignore_content).ok();
        }

        let vc = Self { repo };
        vc.disable_eol_conversion();
        Ok(vc)
    }

    /// Open an existing git repository in the project folder
    pub fn open(project_path: &Path) -> Result<Self, GitError> {
        let repo = Repository::open(project_path)?;
        let vc = Self { repo };
        // Also applied on open so repositories created before this fix are
        // repaired the next time the project is used.
        vc.disable_eol_conversion();
        Ok(vc)
    }

    /// Turn off git's end-of-line conversion for this repository.
    ///
    /// On Windows, `core.autocrlf` defaults to true: git stores LF and
    /// rewrites files to CRLF on checkout. Every file in a project repo is
    /// machine-generated with LF line endings, so after "Restore tune to this
    /// version" checked out a CRLF copy, the very next save rewrote it as LF
    /// and the tree showed as modified without any real change — which in turn
    /// made the SAFE checkout strategy silently skip the *next* restore. A
    /// repo-local `core.autocrlf=false` overrides the user's global setting
    /// for this repo only, so checkouts reproduce the committed bytes exactly.
    /// Best-effort: a failure here degrades to the pre-fix behaviour rather
    /// than breaking init/open.
    fn disable_eol_conversion(&self) {
        if let Ok(mut config) = self.repo.config() {
            let _ = config.set_bool("core.autocrlf", false);
        }
    }

    /// Check if a project folder has a git repository
    pub fn is_git_repo(project_path: &Path) -> bool {
        project_path.join(".git").is_dir()
    }

    /// Open or initialize a git repository
    pub fn open_or_init(project_path: &Path) -> Result<Self, GitError> {
        if Self::is_git_repo(project_path) {
            Self::open(project_path)
        } else {
            Self::init(project_path)
        }
    }

    /// Commit the current tune with a message
    pub fn commit(&self, message: &str) -> Result<String, GitError> {
        let mut index = self.repo.index()?;

        // Add all files (respecting .gitignore)
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let signature = self.get_signature()?;

        // Get parent commit if exists
        let parent_commit = self.get_head_commit();

        let commit_id = if let Ok(parent) = parent_commit {
            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &[&parent],
            )?
        } else {
            // Initial commit
            self.repo
                .commit(Some("HEAD"), &signature, &signature, message, &tree, &[])?
        };

        Ok(commit_id.to_string()[..7].to_string())
    }

    /// Check if there are uncommitted changes
    pub fn has_changes(&self) -> Result<bool, GitError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        Ok(!statuses.is_empty())
    }

    /// Get commit history (most recent first)
    pub fn get_history(&self, max_count: usize) -> Result<Vec<CommitInfo>, GitError> {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(_) => return Ok(vec![]), // No commits yet
        };

        let head_oid = head
            .target()
            .ok_or_else(|| GitError::from_str("HEAD has no target"))?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(head_oid)?;
        revwalk.set_sorting(git2::Sort::TIME)?;

        let mut commits = Vec::new();
        for (i, oid_result) in revwalk.enumerate() {
            if i >= max_count {
                break;
            }

            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;
            let is_head = i == 0;

            commits.push(self.commit_to_info(&commit, is_head));
        }

        Ok(commits)
    }

    /// Get diff between two commits
    pub fn diff_commits(&self, from_sha: &str, to_sha: &str) -> Result<CommitDiff, GitError> {
        let from_oid = self.repo.revparse_single(from_sha)?.id();
        let to_oid = self.repo.revparse_single(to_sha)?.id();

        let from_commit = self.repo.find_commit(from_oid)?;
        let to_commit = self.repo.find_commit(to_oid)?;

        let from_tree = from_commit.tree()?;
        let to_tree = to_commit.tree()?;

        let mut diff_opts = DiffOptions::new();
        let diff =
            self.repo
                .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut diff_opts))?;

        let mut files_changed = Vec::new();
        let mut changes = Vec::new();

        diff.foreach(
            &mut |delta, _| {
                if let Some(path) = delta.new_file().path() {
                    files_changed.push(path.to_string_lossy().to_string());
                }
                true
            },
            None,
            None,
            None,
        )?;

        // For MSQ files, we could parse and compare constants
        // For now, just report file-level changes
        for file in &files_changed {
            if file.ends_with(".msq") || file.ends_with(".json") {
                changes.push(TuneChange {
                    name: file.clone(),
                    old_value: Some(from_sha[..7.min(from_sha.len())].to_string()),
                    new_value: Some(to_sha[..7.min(to_sha.len())].to_string()),
                    change_type: "modified".to_string(),
                });
            }
        }

        Ok(CommitDiff {
            from_sha: from_sha.to_string(),
            to_sha: to_sha.to_string(),
            changes,
            files_changed,
        })
    }

    /// Checkout a specific commit (detached HEAD)
    pub fn checkout_commit(&self, sha: &str) -> Result<(), GitError> {
        self.ensure_clean_for_checkout()?;
        let obj = self.repo.revparse_single(sha)?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head_detached(obj.id())?;
        Ok(())
    }

    /// Refuse a checkout when the working tree has uncommitted changes.
    ///
    /// `checkout_tree` runs libgit2's default SAFE strategy, which skips files
    /// that differ locally — and returns `Ok` when it does. Without this guard
    /// a restore over unsaved changes silently left the old tune in place while
    /// reporting success, and the caller then reloaded and displayed the
    /// *unrestored* file as though the restore had worked. Failing loudly lets
    /// the caller tell the user to save or discard first.
    fn ensure_clean_for_checkout(&self) -> Result<(), GitError> {
        if self.has_changes()? {
            return Err(GitError::from_str(
                "The project has uncommitted changes. Save or discard them before \
                 restoring a previous version, otherwise the restore would be \
                 silently skipped.",
            ));
        }
        Ok(())
    }

    /// Checkout a branch
    pub fn checkout_branch(&self, branch_name: &str) -> Result<(), GitError> {
        // Same SAFE-strategy silent-skip hazard as checkout_commit.
        self.ensure_clean_for_checkout()?;
        let branch = self.repo.find_branch(branch_name, BranchType::Local)?;
        let refname = branch
            .get()
            .name()
            .ok_or_else(|| GitError::from_str("Invalid branch reference"))?;

        let obj = self.repo.revparse_single(refname)?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head(refname)?;
        Ok(())
    }

    /// Create a new branch at current HEAD
    pub fn create_branch(&self, name: &str) -> Result<(), GitError> {
        let head = self.get_head_commit()?;
        self.repo.branch(name, &head, false)?;
        Ok(())
    }

    /// List all local branches
    pub fn list_branches(&self) -> Result<Vec<BranchInfo>, GitError> {
        let mut branches = Vec::new();

        let current_branch = self.get_current_branch_name();

        for branch_result in self.repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("(unnamed)").to_string();

            let tip_sha = branch
                .get()
                .target()
                .map(|oid| oid.to_string()[..7].to_string())
                .unwrap_or_default();

            let is_current = current_branch.as_ref() == Some(&name);

            branches.push(BranchInfo {
                name,
                is_current,
                tip_sha,
            });
        }

        Ok(branches)
    }

    /// Get the current branch name (None if detached HEAD)
    pub fn get_current_branch_name(&self) -> Option<String> {
        let head = self.repo.head().ok()?;
        if head.is_branch() {
            head.shorthand().map(|s| s.to_string())
        } else {
            None
        }
    }

    /// Switch to a branch (must exist)
    pub fn switch_branch(&self, name: &str) -> Result<(), GitError> {
        self.checkout_branch(name)
    }

    // Helper methods

    fn get_signature(&self) -> Result<Signature<'_>, GitError> {
        // Try to get from git config, fall back to defaults
        let config = self.repo.config()?;

        let name = config
            .get_string("user.name")
            .unwrap_or_else(|_| "LibreTune User".to_string());
        let email = config
            .get_string("user.email")
            .unwrap_or_else(|_| "user@libretune.local".to_string());

        Signature::now(&name, &email)
    }

    fn get_head_commit(&self) -> Result<Commit<'_>, GitError> {
        let head = self.repo.head()?;
        let oid = head
            .target()
            .ok_or_else(|| GitError::from_str("HEAD has no target"))?;
        self.repo.find_commit(oid)
    }

    fn commit_to_info(&self, commit: &Commit<'_>, is_head: bool) -> CommitInfo {
        let sha = commit.id().to_string();
        let sha_short = sha[..7.min(sha.len())].to_string();

        let full_message = commit.message().unwrap_or("");
        let message = full_message.lines().next().unwrap_or("").to_string();
        let annotation = extract_annotation(full_message);

        let author = commit.author().name().unwrap_or("Unknown").to_string();

        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        CommitInfo {
            sha_short,
            sha,
            message,
            annotation,
            author,
            timestamp,
            is_head,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Restoring a version must reproduce the committed bytes exactly, keep
    /// the tree clean, and stay repeatable. With the user's global
    /// `core.autocrlf=true` (the Windows default) checkout used to rewrite LF
    /// as CRLF; the next save wrote LF back, the tree showed modified, and —
    /// because SAFE checkout skips modified files while returning Ok — every
    /// restore after the first silently did nothing.
    #[test]
    fn test_checkout_restores_exact_bytes_and_stays_repeatable() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path();
        let vc = VersionControl::init(path).unwrap();

        // Repo-local EOL conversion must be off regardless of global config.
        let cfg = vc.repo.config().unwrap();
        assert!(
            !cfg.get_bool("core.autocrlf").unwrap_or(true),
            "init must set repo-local core.autocrlf=false"
        );

        let f = path.join("CurrentTune.msq");
        let version_a = b"<msq>\n<constant name=\"reqFuel\">12.5</constant>\n</msq>\n";
        std::fs::write(&f, version_a).unwrap();
        vc.commit("A").unwrap();
        let sha_a = vc.get_history(10).unwrap()[0].sha.clone();

        std::fs::write(
            &f,
            b"<msq>\n<constant name=\"reqFuel\">20</constant>\n</msq>\n",
        )
        .unwrap();
        vc.commit("B").unwrap();

        // First restore: exact bytes, clean tree.
        vc.checkout_commit(&sha_a).unwrap();
        assert_eq!(
            std::fs::read(&f).unwrap(),
            version_a,
            "checkout must reproduce the committed bytes (no EOL rewriting)"
        );
        assert!(
            !vc.has_changes().unwrap(),
            "tree must be clean after checkout"
        );

        // Rewriting identical bytes (a reload's save-back) must not dirty it,
        // and a second restore must still work.
        std::fs::write(&f, version_a).unwrap();
        assert!(!vc.has_changes().unwrap());
        vc.checkout_commit(&sha_a)
            .expect("second restore must succeed");

        // Genuine unsaved edits must refuse loudly, not silently skip.
        std::fs::write(&f, b"edited").unwrap();
        let err = vc
            .checkout_commit(&sha_a)
            .expect_err("checkout over unsaved changes must fail, not silently skip");
        assert!(
            err.message().contains("uncommitted"),
            "error should tell the user why: {}",
            err.message()
        );
    }

    #[test]
    fn test_init_and_commit() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        // Initialize repo
        let vc = VersionControl::init(project_path).expect("Failed to init repo");

        // Create a test file
        std::fs::write(project_path.join("test.txt"), "Hello, World!").unwrap();

        // Commit
        let sha = vc.commit("Initial commit").expect("Failed to commit");
        assert_eq!(sha.len(), 7);

        // Check history
        let history = vc.get_history(10).expect("Failed to get history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].message, "Initial commit");
        assert!(history[0].is_head);
    }

    #[test]
    fn test_branch_operations() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        let vc = VersionControl::init(project_path).expect("Failed to init repo");

        // Create a file and initial commit
        std::fs::write(project_path.join("tune.msq"), "<msq/>").unwrap();
        vc.commit("Initial tune").expect("Failed to commit");

        // Create a branch
        vc.create_branch("experiment")
            .expect("Failed to create branch");

        // List branches
        let branches = vc.list_branches().expect("Failed to list branches");
        assert_eq!(branches.len(), 2); // main/master + experiment

        // Find experiment branch
        let exp_branch = branches.iter().find(|b| b.name == "experiment");
        assert!(exp_branch.is_some());
    }

    #[test]
    fn test_is_git_repo() {
        let temp_dir = TempDir::new().unwrap();
        let project_path = temp_dir.path();

        assert!(!VersionControl::is_git_repo(project_path));

        VersionControl::init(project_path).expect("Failed to init repo");

        assert!(VersionControl::is_git_repo(project_path));
    }
}
