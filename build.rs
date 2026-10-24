//! Records the Git commit the program is built from, so the app can show
//! it next to the version. Uses only the standard library.
//!
//! The revision is the short commit hash, with `-dirty` appended when
//! tracked files have uncommitted changes. It is `unknown` when Git or the
//! repository is unavailable, for example when building from a ZIP.

use std::path::Path;
use std::process::Command;

const REVISION_VARIABLE: &str = "MOTUSBOLTZ_REVISION";
const UNKNOWN_REVISION: &str = "unknown";
/// Paths whose changes can alter the revision: the checked-out commit,
/// the staged files, branch tips, and the sources.
const WATCHED_PATHS: [&str; 6] = [
    ".git/HEAD",
    ".git/index",
    ".git/refs/heads",
    ".git/packed-refs",
    "src",
    "tests",
];

fn main() {
    // `--exclude=*` ignores tags, so the result is the hash, not a tag name.
    let revision = Command::new("git")
        .args([
            "describe",
            "--always",
            "--dirty",
            "--abbrev=7",
            "--exclude=*",
        ])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty() && text.is_ascii())
        .unwrap_or_else(|| UNKNOWN_REVISION.to_owned());
    println!("cargo::rustc-env={REVISION_VARIABLE}={revision}");

    println!("cargo::rerun-if-changed=build.rs");
    // A path that does not exist would make Cargo rerun this script on
    // every build, so only existing paths are watched.
    for path in WATCHED_PATHS
        .into_iter()
        .filter(|path| Path::new(path).exists())
    {
        println!("cargo::rerun-if-changed={path}");
    }
}
