//! Workspace automation for fyrer.
//!
//! Entry points (via the `cargo xtask` alias):
//!   cargo xtask build [--release] [--target <triple>]
//!   cargo xtask publish [--dry-run] [--allow-dirty]
//!   cargo xtask bump <version>
//!
//! `publish` pushes member crates to crates.io in dependency order:
//! core -> {process, log, config, cache} -> engine -> {watch, ui} -> fyrer.
use std::{
    env,
    process::{Command, exit},
};

/// Publish order must respect internal dependencies (leaf crates first).
const CRATES: &[&str] = &[
    "fyrer-core",
    "fyrer-process",
    "fyrer-log",
    "fyrer-config",
    "fyrer-cache",
    "fyrer-engine",
    "fyrer-watch",
    "fyrer-ui",
    "fyrer",
];

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        help();
        exit(2);
    };
    let rest: Vec<&str> = args[1..].iter().map(String::as_str).collect();
    match cmd.as_str() {
        "build" => build(&rest),
        "publish" => publish(&rest),
        "bump" => bump(&rest),
        "-h" | "--help" | "help" => help(),
        other => {
            eprintln!("unknown command: {other}\n");
            help();
            exit(2);
        }
    }
}

fn help() {
    println!(
        "fyrer workspace automation

USAGE:
  cargo xtask build [--release] [--target <triple>]
      Build the fyrer CLI (debug or release, optional cross target).

  cargo xtask publish [--dry-run] [--allow-dirty]
      Publish all workspace crates to crates.io in dependency order.
      Already-published versions are skipped automatically.

  cargo xtask bump <version>
      Set [workspace.package].version and sync npm/package.json."
    );
}

fn has_flag<'a>(args: &'a [&'a str], long: &str) -> bool {
    args.iter().any(|a| *a == long)
}

fn value_of<'a>(args: &'a [&'a str], long: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| *a == long)
        .and_then(|i| args.get(i + 1))
        .copied()
}

fn build(args: &[&str]) {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg("fyrer");
    if has_flag(args, "--release") || has_flag(args, "-r") {
        cmd.arg("--release");
    }
    if let Some(target) = value_of(args, "--target") {
        cmd.arg("--target").arg(target);
    }
    run(&mut cmd.status());
}

fn publish(args: &[&str]) {
    let dry = has_flag(args, "--dry-run") || has_flag(args, "-n");
    let dirty = has_flag(args, "--allow-dirty");

    println!(
        "== publishing {} in dependency order{} ==",
        CRATES.len(),
        if dry { " (dry-run)" } else { "" }
    );
    for name in CRATES {
        print!("{name:<14} ");
        let status = publish_one(name, dry, dirty);
        match status {
            PublishOutcome::Ok => println!("OK"),
            PublishOutcome::AlreadyPublished => println!("SKIP (already published)"),
            PublishOutcome::DepNotOnRegistry => {
                if dry {
                    // Expected: dry-run resolves dependencies against the real
                    // index, which only has parents that were already published.
                    println!("SKIP (dry-run: dep not yet on crates.io)");
                } else {
                    println!("FAILED (dependency missing from registry after retries)");
                    exit(1);
                }
            }
            PublishOutcome::Failed(stderr) => {
                println!("FAILED\n{}", indent(&stderr));
                exit(1);
            }
        }
    }
    println!("== done ==");
    if !dry {
        println!("\nnext steps:");
        println!("  git tag v<version> && git push --tags   # triggers binary release CI");
    }
}

enum PublishOutcome {
    Ok,
    AlreadyPublished,
    /// Dependency version not visible on crates.io yet (index propagation lag).
    DepNotOnRegistry,
    Failed(String),
}

fn publish_one(name: &str, dry: bool, dirty: bool) -> PublishOutcome {
    let mut attempts = 0;
    loop {
        attempts += 1;
        let mut cmd = Command::new("cargo");
        cmd.arg("publish").arg("-p").arg(name);
        if dry {
            cmd.arg("--dry-run");
        }
        if dirty {
            cmd.arg("--allow-dirty");
        }
        let output = match cmd.output() {
            Ok(o) => o,
            Err(e) => return PublishOutcome::Failed(format!("failed to spawn cargo: {e}")),
        };
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if output.status.success() {
            return PublishOutcome::Ok;
        }
        if already_published(&stderr) {
            return PublishOutcome::AlreadyPublished;
        }
        if stderr.contains("no matching package named") && !dry && attempts < 6 {
            // Freshly published parent may take a moment to appear in the
            // sparse index; retry with backoff.
            std::thread::sleep(std::time::Duration::from_secs(3));
            continue;
        }
        if stderr.contains("no matching package named") {
            return PublishOutcome::DepNotOnRegistry;
        }
        return PublishOutcome::Failed(stderr);
    }
}

fn already_published(stderr: &str) -> bool {
    stderr.contains("already uploaded")
        || stderr.contains("already exists")
        || stderr.contains("is already registered")
}

fn bump(args: &[&str]) {
    let Some(new_version) = args.first() else {
        eprintln!("usage: cargo xtask bump <version>");
        exit(2);
    };
    if !new_version
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.')
        || new_version.split('.').count() != 3
    {
        eprintln!("invalid version {new_version:?}, expected semver like 0.4.0");
        exit(2);
    }

    // 1. Root Cargo.toml [workspace.package].version
    replace_in_file(
        "Cargo.toml",
        |line| line.starts_with("version = ") && in_workspace_package(line),
        |_| format!("version = \"{new_version}\""),
    );

    // 2. workspace.dependencies version pins
    replace_in_file("Cargo.toml", |line| line.contains("crates/fyrer-"), |line| {
        re_pin_version(line, new_version)
    });

    // 3. npm/package.json
    replace_in_file("npm/package.json", |line| line.trim_start().starts_with("\"version\""), |_| {
        format!("  \"version\": \"{new_version}\",")
    });

    println!("bumped to {new_version}");
    println!("\nremaining manual steps:");
    println!("  - update version references in install.sh / install.ps1 if pinned");
    println!("  - commit, then: cargo xtask publish");
}

// --- helpers ---------------------------------------------------------------

fn run(result: &std::io::Result<std::process::ExitStatus>) {
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => exit(status.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("failed to spawn cargo: {e}");
            exit(1);
        }
    }
}

fn in_workspace_package(_line: &str) -> bool {
    // The first `version = "..."` line in Cargo.toml belongs to
    // [workspace.package]; the [package] one uses `version.workspace = true`.
    true
}

fn re_pin_version(line: &str, version: &str) -> String {
    // fyrer-x = { path = "...", version = "OLD" } -> version = "<new>"
    if let Some(idx) = line.find("version = \"") {
        let start = idx + "version = \"".len();
        if let Some(end_rel) = line[start..].find('"') {
            let end = start + end_rel;
            return format!("{}{}{}", &line[..start], version, &line[end..]);
        }
    }
    line.to_string()
}

fn replace_in_file<F>(path: &str, matches: F, transform: impl Fn(&str) -> String)
where
    F: Fn(&str) -> bool,
{
    let full = std::path::Path::new(path).to_path_buf();
    let content = std::fs::read_to_string(&full).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        exit(1);
    });
    let mut out = Vec::new();
    let mut changed = 0;
    for line in content.lines() {
        if matches(line) {
            out.push(transform(line));
            changed += 1;
        } else {
            out.push(line.to_string());
        }
    }
    if changed == 0 && path.ends_with("package.json") {
        eprintln!("warning: no lines matched in {path}");
    }
    std::fs::write(
        &full,
        out.join("\n") + if content.ends_with('\n') { "\n" } else { "" },
    )
    .unwrap_or_else(|e| {
        eprintln!("cannot write {path}: {e}");
        exit(1);
    });
}

fn indent(s: &str) -> String {
    s.lines()
        .map(|l| format!("    {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}
