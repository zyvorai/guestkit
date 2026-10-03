// Builds `guestkit-ebpf` for bpfel-unknown-none and stages the object at
// `$OUT_DIR/guestkit-ebpf.o` for `include_bytes_aligned!`.
//
// The kernel crate needs nightly + `bpf-linker`. When they are missing (or on
// non-Linux hosts, or with GUESTKIT_EBPF_SKIP=1) an empty object is staged and
// the agent reports `programs_compiled: false` instead of failing the build.
// GUESTKIT_EBPF_OBJ=/path/to/obj stages a prebuilt object.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn find_tool(name: &str) -> Option<PathBuf> {
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let home = env::var_os("HOME")?;
    let p = Path::new(&home).join(".cargo/bin").join(name);
    p.is_file().then_some(p)
}

fn stage_empty(dst: &Path, why: &str) {
    println!("cargo:warning=guestkit-ebpf: programs not built ({why}); netpolicy/lsm will be unavailable");
    fs::write(dst, b"").expect("write empty bpf object");
}

/// Cargo treats a missing `rerun-if-changed` path as always changed, so
/// watching where the tool would be installed re-runs this script until it
/// appears instead of caching the empty object forever.
fn watch_install_paths(name: &str) {
    let mut dirs: Vec<PathBuf> = env::var_os("PATH")
        .map(|p| env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = env::var_os("HOME") {
        dirs.push(Path::new(&home).join(".cargo/bin"));
    }
    dirs.push(PathBuf::from("/usr/local/bin"));
    for d in dirs {
        println!("cargo:rerun-if-changed={}", d.join(name).display());
    }
}

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let dst = out.join("guestkit-ebpf.o");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let ebpf_dir = manifest.join("../guestkit-ebpf");
    let common_dir = manifest.join("../guestkit-ebpf-common");

    for v in ["GUESTKIT_EBPF_OBJ", "GUESTKIT_EBPF_SKIP", "GUESTKIT_EBPF_TOOLCHAIN"] {
        println!("cargo:rerun-if-env-changed={v}");
    }
    for d in [ebpf_dir.join("src"), ebpf_dir.join("Cargo.toml"), common_dir.join("src")] {
        println!("cargo:rerun-if-changed={}", d.display());
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux") {
        fs::write(&dst, b"").expect("write empty bpf object");
        return;
    }
    if let Some(obj) = env::var_os("GUESTKIT_EBPF_OBJ") {
        fs::copy(&obj, &dst).expect("copy GUESTKIT_EBPF_OBJ");
        return;
    }
    if matches!(env::var("GUESTKIT_EBPF_SKIP").as_deref(), Ok("1") | Ok("true")) {
        stage_empty(&dst, "GUESTKIT_EBPF_SKIP set");
        return;
    }
    let toolchain = env::var("GUESTKIT_EBPF_TOOLCHAIN").unwrap_or_else(|_| "nightly".into());
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".into());
    if find_tool("bpf-linker").is_none() {
        watch_install_paths("bpf-linker");
        stage_empty(&dst, "bpf-linker not found (cargo install bpf-linker)");
        return;
    }
    let Some(rustup) = find_tool("rustup") else {
        watch_install_paths("rustup");
        stage_empty(&dst, "rustup not found (nightly toolchain required)");
        return;
    };
    let target_dir = out.join("ebpf-target");

    let rustflags = [
        format!("--cfg=bpf_target_arch=\"{arch}\""),
        "-Cdebuginfo=2".into(),
        "-Clink-arg=--btf".into(),
    ]
    .join("\x1f");

    let mut cmd = Command::new(&rustup);
    cmd.current_dir(&ebpf_dir)
        .args(["run", &toolchain, "cargo", "build", "--release"])
        .args(["--target", "bpfel-unknown-none", "-Z", "build-std=core"])
        .arg("--target-dir")
        .arg(&target_dir)
        .env("CARGO_ENCODED_RUSTFLAGS", rustflags);
    for k in [
        "RUSTC",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTC_WRAPPER",
        "RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "CARGO_MANIFEST_DIR",
        "CARGO_PKG_NAME",
    ] {
        cmd.env_remove(k);
    }
    let output = cmd.output().expect("spawn rustup cargo build for guestkit-ebpf");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("toolchain") && stderr.contains("not installed") {
            stage_empty(&dst, &format!("rustup toolchain `{toolchain}` not installed"));
            return;
        }
        panic!("guestkit-ebpf build failed:\n{stderr}");
    }
    let built = target_dir.join("bpfel-unknown-none/release/guestkit-ebpf");
    fs::copy(&built, &dst).unwrap_or_else(|e| panic!("copy {}: {e}", built.display()));
}
