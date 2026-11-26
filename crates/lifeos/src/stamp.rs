//! What `lifeos --version` says about the commit a binary was built from. `build.rs` asks git and calls
//! `stamp` at build time; `tests/version.rs` includes this file too, for its tests and for `STATUS`.

/// The stamp of a build git names no commit for.
pub const UNKNOWN: &str = "commit unknown";

/// The git arguments that list what differs from HEAD, one line per changed, staged or untracked file.
/// `--untracked-files=normal` is said outright: left to git's configuration, `status.showUntrackedFiles=no`
/// hides an untracked source file and the build reads clean. Without optional locks, so a build does
/// not hold the index against a git command run beside it.
pub const STATUS: &[&str] = &[
    "--no-optional-locks",
    "status",
    "--porcelain",
    "--untracked-files=normal",
];

/// The stamp, from what git answered. `head` is the output of `git rev-parse --verify HEAD^{commit}` and
/// `status` that of git run with `STATUS`, each `None` when the command could not run or failed.
///
/// `<id>` only when git named a commit and said the tree is clean; `<id>-dirty` when the tree differs
/// from that commit or git could not say; `commit unknown` when git named no commit.
pub fn stamp(head: Option<&str>, status: Option<&str>) -> String {
    let Some(id) = head.map(str::trim).filter(|id| is_commit_id(id)) else {
        return UNKNOWN.to_owned();
    };
    if status.is_some_and(|changes| changes.trim().is_empty()) {
        id.to_owned()
    } else {
        format!("{id}-dirty")
    }
}

/// A full object id as git prints it: 40 hex digits (SHA-1) or 64 (SHA-256), lower case.
fn is_commit_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::{STATUS, stamp};
    use std::path::Path;
    use std::process::{Command, Output};

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";

    /// Git run in `repo` and nowhere else, told by its configuration to hide untracked files.
    #[expect(
        clippy::disallowed_methods,
        reason = "a test of the git arguments starts git directly: short commands that end by themselves"
    )]
    fn git_hiding_untracked(repo: &Path, args: &[&str]) -> std::io::Result<Output> {
        Command::new("git")
            .args(args)
            .current_dir(repo)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_INDEX_FILE")
            .env_remove("GIT_OBJECT_DIRECTORY")
            .env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "status.showUntrackedFiles")
            .env("GIT_CONFIG_VALUE_0", "no")
            .output()
    }

    /// A repository of its own in the temp directory, never the checkout: one untracked file, and a git
    /// configured to hide it. Plain `status --porcelain` then prints nothing; `STATUS` must still list it.
    #[test]
    fn status_lists_an_untracked_file_git_is_configured_to_hide() {
        let nanos = std::time::UNIX_EPOCH
            .elapsed()
            .map_or(0, |since| since.as_nanos());
        let repo =
            std::env::temp_dir().join(format!("lifeos-stamp-{}-{nanos}", std::process::id()));
        std::fs::create_dir(&repo).expect("a new temp directory");
        let read = || {
            // No git on PATH: nothing to check, and a build there says `commit unknown` anyway.
            let init = git_hiding_untracked(&repo, &["init", "--quiet"]).ok()?;
            assert!(init.status.success(), "git init: {init:?}");
            std::fs::write(repo.join("untracked.rs"), "").expect("a file in the temp repository");
            let plain =
                git_hiding_untracked(&repo, &["status", "--porcelain"]).expect("git status");
            let ours = git_hiding_untracked(&repo, STATUS).expect("git status");
            Some((plain, ours))
        };
        let read = std::panic::catch_unwind(read);
        std::fs::remove_dir_all(&repo).expect("the temp repository is removed");
        let Some((plain, ours)) = read.expect("git ran in the temp repository") else {
            return;
        };
        assert!(
            plain.status.success() && ours.status.success(),
            "{plain:?} {ours:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&plain.stdout),
            "",
            "the configuration under test hides untracked files from a plain `git status --porcelain`"
        );
        assert_eq!(
            String::from_utf8_lossy(&ours.stdout),
            "?? untracked.rs\n",
            "`STATUS` lists an untracked file whatever git's configuration says"
        );
    }

    #[test]
    fn a_clean_tree_is_the_commit_id_alone() {
        assert_eq!(stamp(Some(&format!("{ID}\n")), Some("")), ID);
        assert_eq!(stamp(Some(ID), Some("\n")), ID);
    }

    #[test]
    fn a_changed_tree_is_marked_dirty() {
        assert_eq!(
            stamp(Some(ID), Some(" M crates/lifeos/src/main.rs\n")),
            format!("{ID}-dirty")
        );
        assert_eq!(
            stamp(Some(ID), Some("?? crates/lifeos/src/new.rs\n")),
            format!("{ID}-dirty")
        );
    }

    #[test]
    fn a_tree_git_could_not_read_is_not_called_clean() {
        assert_eq!(stamp(Some(ID), None), format!("{ID}-dirty"));
    }

    #[test]
    fn no_commit_is_unknown_whatever_the_tree() {
        assert_eq!(stamp(None, None), "commit unknown");
        assert_eq!(stamp(None, Some("")), "commit unknown");
    }

    #[test]
    fn only_a_full_object_id_is_a_commit() {
        let sha256 = ID.repeat(2).split_at(64).0.to_owned();
        assert_eq!(stamp(Some(&sha256), Some("")), sha256);
        for not_an_id in [
            "",
            "HEAD",
            &ID[..7],
            &ID[..39],
            &ID.to_uppercase(),
            "fatal: not a git repository",
        ] {
            assert_eq!(
                stamp(Some(not_an_id), Some("")),
                "commit unknown",
                "{not_an_id:?}"
            );
        }
        let two_lines = format!("{ID}\n{ID}\n");
        assert_eq!(stamp(Some(&two_lines), Some("")), "commit unknown");
    }
}
