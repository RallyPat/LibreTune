//! Git-based version control for tune files
//!
//! Provides local git integration for tracking tune changes over time,
//! allowing users to view history, compare versions, and restore previous tunes.

use git2::{
    BranchType, Commit, DiffOptions, Error as GitError, IndexAddOption, Repository,
    Signature, StatusOptions,
};
use std::path::Path;

/// Information about a commit in the tune history
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommitInfo {
    /// Short SHA (first 7 characters)
    pub sha_short: String,
    /// Full SHA hash
    pub sha: String,
    /// Commit message
    pub message: String,
    /// Author name
    pub author: String,
    /// Commit timestamp (ISO 8601)
    pub timestamp: String,
    /// Whether this is the current HEAD
    pub is_head: bool,
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

        Ok(Self { repo })
    }

    /// Open an existing git repository in the project folder
    pub fn open(project_path: &Path) -> Result<Self, GitError> {
        let repo = Repository::open(project_path)?;
        Ok(Self { repo })
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

        let head_oid = head.target().ok_or_else(|| {
            GitError::from_str("HEAD has no target")
        })?;

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
        let diff = self
            .repo
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
        let obj = self.repo.revparse_single(sha)?;
        self.repo.checkout_tree(&obj, None)?;
        self.repo.set_head_detached(obj.id())?;
        Ok(())
    }

    /// Checkout a branch
    pub fn checkout_branch(&self, branch_name: &str) -> Result<(), GitError> {
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
            let name = branch
                .name()?
                .unwrap_or("(unnamed)")
                .to_string();

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
        let oid = head.target().ok_or_else(|| {
            GitError::from_str("HEAD has no target")
        })?;
        self.repo.find_commit(oid)
    }

    fn commit_to_info(&self, commit: &Commit<'_>, is_head: bool) -> CommitInfo {
        let sha = commit.id().to_string();
        let sha_short = sha[..7.min(sha.len())].to_string();

        let message = commit
            .message()
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        let author = commit.author().name().unwrap_or("Unknown").to_string();

        let time = commit.time();
        let timestamp = chrono::DateTime::from_timestamp(time.seconds(), 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        CommitInfo {
            sha_short,
            sha,
            message,
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
        vc.create_branch("experiment").expect("Failed to create branch");

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
