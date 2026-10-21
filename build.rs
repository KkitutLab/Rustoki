use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn main() {
    // Re-run when the git state may have changed.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/tags/");
    println!("cargo:rerun-if-changed=build.rs");

    // e.g. "1.0.0", "1.0.0-3-gf45f618", "f45f618-dirty" (no tag yet), or "unknown".
    let describe =
        git(&["describe", "--tags", "--always", "--dirty"]).unwrap_or_else(|| "unknown".into());
    let commit = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=GIT_DESCRIBE={describe}");
    println!("cargo:rustc-env=GIT_COMMIT={commit}");
}
