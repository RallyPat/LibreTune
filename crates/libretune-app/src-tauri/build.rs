use chrono::Utc;
use std::process::Command;

fn get_git_short_sha() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sha.is_empty() {
        None
    } else {
        Some(sha)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");

    let date = Utc::now().format("%Y.%m.%d").to_string();
    let sha = get_git_short_sha().unwrap_or_else(|| "unknown".to_string());
    let build_id = format!("{date}+g{sha}");

    println!("cargo:rustc-env=LIBRETUNE_BUILD_ID={build_id}");

    tauri_build::build()
}
