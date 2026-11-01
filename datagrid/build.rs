// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Writes the .rgr classes the crate includes with `ranger::import_rgr!`
// (src/rgr.rs: src/DgRgr.rgr and what it imports) as a Rust module into
// OUT_DIR, with the Ranger checkout's compiler (RANGER_HOME, else the
// checkout this directory sits in at gallery/datagrid). Needs node.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let ranger = std::env::var("RANGER_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| manifest.join("../.."));
    let rgrc = ranger.join("dist/rgrc.js");
    let out = std::env::var("OUT_DIR").unwrap();
    let run = Command::new("node")
        .arg("--max-old-space-size=8192")
        .arg(&rgrc)
        .args(["-l=rust", "-rust-module"])
        .arg(manifest.join("src/DgRgr.rgr"))
        .arg(format!("-d={}", out))
        .arg("-o=DgRgr.rs")
        .current_dir(&ranger)
        .output()
        .expect("node could not run Ranger's rgrc");
    let log = String::from_utf8_lossy(&run.stdout).to_string() + &String::from_utf8_lossy(&run.stderr);
    if !run.status.success() || log.contains("[FAIL]") {
        panic!("rgrc could not compile src/DgRgr.rgr ({}):\n{}", rgrc.display(), log);
    }
    println!("cargo:rerun-if-changed=src/DgRgr.rgr");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RANGER_HOME");
    println!("cargo:rerun-if-changed={}", ranger.join("lib/evg").display());
}
