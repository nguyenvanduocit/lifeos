//! What `lifeos --version` says about the commit a binary was built from. `build.rs` asks git and calls
//! `stamp` at build time; `main.rs` holds this file only to run its tests.

/// The stamp of a build git names no commit for.
pub const UNKNOWN: &str = "commit unknown";

/// The stamp, from what git answered. `head` is the output of `git rev-parse --verify HEAD^{commit}` and
/// `status` that of `git status --porcelain`, each `None` when the command could not run or failed.
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
    use super::stamp;

    const ID: &str = "0123456789abcdef0123456789abcdef01234567";

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
