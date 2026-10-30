// SPDX-License-Identifier: GPL-3.0-or-later
//! Harmless native fixture. Only relative test-control/report files in owned CWD.
use std::{
    fs,
    io::{self, Read, Write},
    time::Duration,
};
fn string(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&(text.len() as u32).to_le_bytes());
    out.extend_from_slice(text.as_bytes());
}
fn main() -> io::Result<()> {
    // Failed stream assertions cannot leave an indefinitely blocked fixture.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(4));
        let _ = fs::write("watchdog.txt", "synthetic fixture exceeded bound");
        std::process::exit(86);
    });
    fs::write("phase.txt", "entered-main")?;
    let control = fs::read_to_string("fixture-control.txt")?;
    let values: Vec<_> = control.split_whitespace().collect();
    let exit: i32 = values[0].parse().unwrap();
    let sleep: u64 = values[1].parse().unwrap();
    assert!(sleep <= 3000);
    let mut report = Vec::new();
    string(&mut report, &std::process::id().to_string());
    string(&mut report, std::env::current_dir()?.to_str().unwrap());
    string(&mut report, std::env::current_exe()?.to_str().unwrap());
    string(
        &mut report,
        &std::env::var("SystemRoot").unwrap_or_default(),
    );
    let mut input = [0];
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
        fn GetFileType(handle: *mut std::ffi::c_void) -> u32;
    }
    fs::write("phase.txt", unsafe {
        format!(
            "before-stdin handle={:?} type={}",
            GetStdHandle(-10i32 as u32),
            GetFileType(GetStdHandle(-10i32 as u32))
        )
    })?;
    string(
        &mut report,
        if matches!(io::stdin().read(&mut input), Ok(0) | Err(_)) {
            "noInput"
        } else {
            "inheritedInput"
        },
    );
    fs::write("phase.txt", "after-stdin")?;
    // Optional inheritable sentinel event: SetEvent, not numeric-handle validity,
    // distinguishes inheritance from an unrelated reused handle value.
    #[cfg(windows)]
    if let Some(value) = values.get(2) {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn SetEvent(event: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            SetEvent(value.parse::<usize>().unwrap() as *mut _);
        }
    }
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    for a in &arguments {
        string(&mut report, a);
    }
    // Stream writes must not appear on the test runner's stdout/stderr.
    let _ = io::stdout().write_all(b"M17_FIXTURE_STDOUT_MUST_NOT_LEAK");
    let _ = io::stderr().write_all(b"M17_FIXTURE_STDERR_MUST_NOT_LEAK");
    fs::write("received.bin", report)?;
    std::thread::sleep(Duration::from_millis(sleep));
    fs::write("exited.txt", exit.to_string())?;
    std::process::exit(exit);
}
