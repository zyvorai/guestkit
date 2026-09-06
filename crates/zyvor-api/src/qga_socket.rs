// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! In-pod QGA transport that does **not** call `virsh`.
//!
//! virt-launcher images ship `virsh`, but GuestKit speaks the QGA unix socket
//! directly (python / perl / socat / nc / guestkit qga). `virsh` is only used
//! when `GUESTKIT_ALLOW_VIRSH=1` is set in the zyvor-api process.

use serde_json::Value;

/// Socket paths and glob roots searched inside virt-launcher.
pub const SOCKET_FIND_SCRIPT: &str = r#"
find_qga_sock() {
  for p in \
    /var/run/kubevirt-private/libvirt/qemu/channel/target/org.qemu.guest_agent.0 \
    /var/run/libvirt/qemu/channel/target/org.qemu.guest_agent.0 \
    /var/lib/libvirt/qemu/channel/target/org.qemu.guest_agent.0 \
    /var/run/kubevirt/guest-agent.sock
  do
    [ -S "$p" ] && echo "$p" && return 0
  done
  find /var/run/kubevirt-private /var/run/libvirt /var/lib/libvirt /var/run/kubevirt \
    -type s 2>/dev/null | grep -E 'guest_agent|org.qemu.guest_agent' | head -n 1
}
"#;

/// Build the argv executed in the virt-launcher `compute` container.
///
/// `$1` is the JSON QGA body. Domain name is only used for the optional virsh
/// fallback so existing callers can keep passing it.
pub fn qga_exec_argv(domain: &str, allow_virsh: bool) -> Vec<String> {
    let fallback = if allow_virsh {
        format!(
            r#"
  if command -v virsh >/dev/null 2>&1; then
    exec virsh --quiet qemu-agent-command {domain} "$1"
  fi
"#
        )
    } else {
        String::new()
    };

    let script = format!(
        r#"
set -e
# libvirt domain name (used only by the optional virsh fallback): {domain}
{find}
SOCK=$(find_qga_sock)
if [ -z "$SOCK" ]; then
  echo "no QGA unix socket found in virt-launcher" >&2
  exit 2
fi

if command -v guestkit >/dev/null 2>&1; then
  exec guestkit qga --socket "$SOCK" --raw "$1"
fi

if command -v python3 >/dev/null 2>&1; then
  exec python3 - "$SOCK" "$1" <<'PY'
import socket, sys
sock, body = sys.argv[1], sys.argv[2]
if not body.endswith("\n"):
    body += "\n"
s = socket.socket(socket.AF_UNIX)
s.settimeout(30)
s.connect(sock)
s.sendall(body.encode())
buf = b""
while True:
    chunk = s.recv(4096)
    if not chunk:
        break
    buf += chunk
    if b"\n" in buf:
        break
sys.stdout.write(buf.decode(errors="replace"))
PY
fi

if command -v python >/dev/null 2>&1; then
  exec python -c 'import socket,sys; sock,body=sys.argv[1],sys.argv[2];
body=body if body.endswith("\n") else body+"\n"
s=socket.socket(socket.AF_UNIX); s.settimeout(30); s.connect(sock); s.sendall(body.encode());
buf=b""
while True:
 c=s.recv(4096)
 if not c: break
 buf+=c
 if b"\n" in buf: break
sys.stdout.write(buf.decode("utf-8","replace"))' "$SOCK" "$1"
fi

if command -v perl >/dev/null 2>&1; then
  exec perl -e '
    use IO::Socket::UNIX;
    my $sock = IO::Socket::UNIX->new(Peer => $ARGV[0]) or die $!;
    $sock->send($ARGV[1] =~ /\n$/ ? $ARGV[1] : $ARGV[1]."\n");
    print <$sock>;
  ' "$SOCK" "$1"
fi

if command -v socat >/dev/null 2>&1; then
  printf "%s\n" "$1" | exec socat -t 30 - "UNIX-CONNECT:$SOCK"
fi

if command -v nc >/dev/null 2>&1; then
  printf "%s\n" "$1" | exec nc -U -w 30 "$SOCK"
fi
{fallback}
echo "no QGA client in virt-launcher (tried guestkit, python, perl, socat, nc). Rebuild virt-launcher with guestkit or set GUESTKIT_ALLOW_VIRSH=1" >&2
exit 3
"#,
        find = SOCKET_FIND_SCRIPT,
        fallback = fallback,
        domain = domain,
    );

    vec!["sh".into(), "-c".into(), script, "--".into()]
}

pub fn allow_virsh_fallback() -> bool {
    matches!(
        std::env::var("GUESTKIT_ALLOW_VIRSH").ok().as_deref(),
        Some("1") | Some("true") | Some("TRUE") | Some("yes")
    )
}

pub fn encode_request(body: &Value) -> Result<String, serde_json::Error> {
    serde_json::to_string(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn argv_does_not_mention_virsh_by_default() {
        let argv = qga_exec_argv("ns_vm", false);
        assert_eq!(argv[0], "sh");
        assert!(!argv[2].contains("qemu-agent-command"));
        assert!(argv[2].contains("guestkit qga"));
        assert!(argv[2].contains("python3"));
    }

    #[test]
    fn argv_can_opt_in_to_virsh() {
        let argv = qga_exec_argv("ns_vm", true);
        assert!(argv[2].contains("virsh --quiet qemu-agent-command ns_vm"));
    }

    #[test]
    fn encode_roundtrip() {
        let s = encode_request(&json!({"execute":"guest-ping"})).unwrap();
        assert_eq!(s, r#"{"execute":"guest-ping"}"#);
    }
}
