use chrono::Local;
use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let now = Local::now();
    let date = now.format("%Y%m%d").to_string();
    let version = format!("{}-{}", git_hash, date);
    println!("cargo:rustc-env=BUILD_VERSION={}", version);
    println!("cargo:rerun-if-changed=build.rs");
}
