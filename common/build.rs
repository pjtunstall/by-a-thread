use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    if let Some(ref c) = commit {
        println!("cargo:rustc-env=BUILD_GIT_COMMIT={}", c);
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/refs/heads");
    } else {
        println!("cargo:rustc-env=BUILD_GIT_COMMIT=");
    }
}
