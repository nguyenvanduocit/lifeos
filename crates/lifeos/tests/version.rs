//! `lifeos --version` names the commit the binary was built from (build.rs), read from the built binary.
//!
//! This test starts the binary and `git` itself: short commands that end by themselves.
#![expect(
    clippy::disallowed_methods,
    reason = "a test of the built binary starts it, and asks git, directly"
)]

use std::process::Command;

// The stamp's own tests, and the `git status` arguments build.rs uses.
#[path = "../src/stamp.rs"]
mod stamp;

/// What git prints for the sources this test was built from; `None` when git is absent or fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[test]
fn version_names_the_commit_the_binary_was_built_from() {
    let output = Command::new(env!("CARGO_BIN_EXE_lifeos"))
        .arg("--version")
        .output()
        .expect("lifeos starts");
    assert!(output.status.success(), "lifeos --version: {output:?}");
    let printed = String::from_utf8(output.stdout).expect("utf-8");
    let printed = printed.trim_end();
    let version = env!("CARGO_PKG_VERSION");

    // Asked of git here, not of build.rs: the commit of the checkout, when git tracks these sources.
    let tracked = git(&["ls-files", "--error-unmatch", "Cargo.toml"]).is_some();
    let head = if tracked {
        git(&["rev-parse", "--verify", "HEAD^{commit}"])
    } else {
        None
    };

    let Some(head) = head else {
        // No git or no checkout (a source tarball): the build works and names no commit.
        assert_eq!(printed, format!("lifeos {version} (commit unknown)"));
        return;
    };
    assert_eq!(head.len(), 40, "git rev-parse printed {head:?}");
    let dirty = git(stamp::STATUS).is_none_or(|changes| !changes.is_empty());
    let mark = if dirty { "-dirty" } else { "" };
    assert_eq!(
        printed,
        format!("lifeos {version} ({head}{mark})"),
        "the binary must name HEAD and say whether the tree differs from it, as they are now: \
         cargo rebuilds the stamp before each test run, so a difference is a stale stamp \
         (or a commit or an edit made while the tests ran)"
    );
}
