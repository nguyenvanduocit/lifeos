//! Stamps the binary with the git commit it is built from: `lifeos --version` prints it (src/main.rs).
//!
//! The stamp is one of:
//! - `<commit id>`: the sources are exactly that commit (40 hex digits);
//! - `<commit id>-dirty`: the working tree differs from that commit, by a changed, staged or untracked
//!   file anywhere in the repository (an untracked file can be built too), or git could not say;
//! - `commit unknown`: git names no commit for these sources. No git on PATH, sources outside a git
//!   checkout (a tarball), a repository with no commit yet, or sources unpacked inside some other
//!   repository that does not track them. The build still succeeds and says so with a warning.
//!
//! The id is never a constant and never one kept from an earlier build: git is asked in every run, and no
//! environment variable sets it (cargo's `rustc-env` wins over the caller's environment, and the
//! variables that point git at another repository are dropped).
//!
//! This script runs on every build of the package, not only when HEAD moves. Watching HEAD and its ref
//! would catch a new commit but not an edit in another crate after a clean build, which would leave a
//! binary that claims a clean commit. Cost: three short git commands, and because cargo rebuilds a
//! package whose build script ran, this small crate is compiled and the binary linked again on every
//! `cargo build`, even when nothing changed: 7 to 11 seconds measured for the debug binary (three
//! builds in a row, no debug info, a 10-core Mac under load), where such a build did nothing before.
//!
//! Limits. The stamp is the tree as cargo last built this package: nothing changes it until the next
//! build. `-dirty` is the whole repository, so an edited document marks the build too. Files git ignores
//! are not seen: `web/dist`, which lifeos-web embeds, is as fresh as the last `make web` (`make build`
//! runs it first).

use std::path::Path;
use std::process::Command;

#[path = "src/stamp.rs"]
mod stamp;

fn main() {
    // A path that never exists: cargo then runs this script on every build (see above).
    println!("cargo:rerun-if-changed=.rerun-on-every-build");

    let sources = std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default();
    let sources = Path::new(&sources);
    // A tarball unpacked inside another repository must not take that repository's commit: the commit
    // counts only when git tracks this package's manifest.
    let tracked = git(sources, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_some();
    let head = if tracked {
        git(sources, &["rev-parse", "--verify", "HEAD^{commit}"])
    } else {
        None
    };
    // Without optional locks: a build must not hold the index against a git command run beside it.
    let status = git(
        sources,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=normal",
        ],
    );

    let stamp = stamp::stamp(head.as_deref(), status.as_deref());
    if stamp == stamp::UNKNOWN {
        println!(
            "cargo:warning=git names no commit for these sources: `lifeos --version` will say `{stamp}`"
        );
    }
    println!("cargo:rustc-env=LIFEOS_BUILD_COMMIT={stamp}");
}

/// What a git command printed, run in the package's directory; `None` when git is absent or fails.
#[expect(
    clippy::disallowed_methods,
    reason = "a build script cannot use procgroup: short git commands that end by themselves"
)]
fn git(sources: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(sources)
        // The repository is the one the sources are in, never one the caller's environment points at.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
