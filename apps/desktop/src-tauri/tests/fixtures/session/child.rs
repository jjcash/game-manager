// SPDX-License-Identifier: GPL-3.0-or-later
//! Harmless fixture. Self-release watchdog; never terminates another process.
#![windows_subsystem = "windows"]
use std::os::windows::process::CommandExt;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let root = Path::new(&args[2]);
    fs::write(root.join("ready"), std::process::id().to_string()).unwrap();
    if args[1] == "handoff" {
        let next = root.join("descendant");
        fs::create_dir(&next).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .arg("hold")
            .arg(&next)
            .arg("0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        fs::write(root.join("descendant-pid"), child.id().to_string()).unwrap();
        drop(child);
    } else if args[1] == "hold" {
        let start = Instant::now();
        while !root.join("release").exists() {
            if start.elapsed() > Duration::from_secs(15) {
                fs::write(root.join("watchdog"), b"self release").unwrap();
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    let code: i32 = args[3].parse().unwrap();
    fs::write(root.join("done"), code.to_string()).unwrap();
    std::process::exit(code);
}
