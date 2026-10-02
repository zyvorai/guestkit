// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! The agent runs unprivileged, so freezing must go through the privileged helper
//! (`guestkitd-exec`). This starts the real helper binary with a fake `fsfreeze` on its PATH
//! and checks that the agent-side call reaches it, with the right arguments, and that a
//! failure of the tool is reported back instead of being swallowed.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

struct Helper(Child);
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_helper(dir: &std::path::Path, fail: bool) -> (Helper, std::path::PathBuf) {
    let sock = dir.join(if fail { "fail.sock" } else { "ok.sock" });
    let bin_dir = dir.join(if fail { "bin-fail" } else { "bin-ok" });
    std::fs::create_dir_all(&bin_dir).unwrap();
    let fake = bin_dir.join("fsfreeze");
    let log = dir.join("args.log");
    std::fs::write(
        &fake,
        format!(
            "#!/bin/sh\necho \"$@\" >> {}\n{}exit 0\n",
            log.display(),
            if fail {
                "echo 'Operation not permitted' >&2; exit 1\n"
            } else {
                ""
            }
        ),
    )
    .unwrap();
    std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let child = Command::new(env!("CARGO_BIN_EXE_guestkitd-exec"))
        .env("ZYVOR_EXEC_SOCKET", &sock)
        .env("PATH", path)
        .spawn()
        .expect("start guestkitd-exec");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !sock.exists() {
        assert!(Instant::now() < deadline, "helper socket never appeared");
        std::thread::sleep(Duration::from_millis(50));
    }
    (Helper(child), sock)
}

#[test]
fn freeze_and_thaw_go_through_the_helper_and_failures_surface() {
    let dir = tempfile::tempdir().unwrap();

    let (_ok, sock) = start_helper(dir.path(), false);
    std::env::set_var("ZYVOR_EXEC_SOCKET", &sock);
    assert!(guestkit::agent::executor_ipc::executor_available());
    guestkit::agent::executor_ipc::fsfreeze(false).expect("freeze via helper");
    guestkit::agent::executor_ipc::fsfreeze(true).expect("thaw via helper");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("args.log")).unwrap(),
        "-f /\n-u /\n",
        "the helper ran fsfreeze, not this process"
    );

    let (_bad, sock) = start_helper(dir.path(), true);
    std::env::set_var("ZYVOR_EXEC_SOCKET", &sock);
    let err = guestkit::agent::executor_ipc::fsfreeze(false)
        .unwrap_err()
        .to_string();
    assert!(err.contains("Operation not permitted"), "{err}");
}
