// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Agent integration tests (requires --features agent).

#![cfg(feature = "agent")]

use guestkit::agent::handler::RequestHandler;

#[test]
fn agent_ping_via_handler() {
    let handler = RequestHandler::new();
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.ping","id":1}"#);
    assert!(resp.result.is_some());
}

#[test]
fn agent_get_capabilities() {
    let handler = RequestHandler::new();
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.getCapabilities","id":2}"#);
    assert!(resp.result.is_some());
}

#[test]
fn agent_get_evidence_live() {
    let handler = RequestHandler::new();
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.getEvidence","id":3}"#);
    assert!(resp.result.is_some(), "{:?}", resp.error);
}

#[test]
fn agent_doctor_live() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.doctor","params":{"target":"kvm"},"id":4}"#,
    );
    assert!(resp.result.is_some(), "{:?}", resp.error);
    let semantic = resp.result.as_ref().and_then(|r| r.get("semantic"));
    assert!(
        semantic.is_some(),
        "doctor should include semantic analysis"
    );
}

#[test]
fn agent_collect_support_bundle() {
    let handler = RequestHandler::new();
    let resp =
        handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.collectSupportBundle","id":5}"#);
    assert!(resp.result.is_some(), "{:?}", resp.error);
    let result = resp.result.as_ref().expect("result");
    assert_eq!(
        result.get("format").and_then(|v| v.as_str()),
        Some("tar.zst")
    );
    assert_eq!(
        result.get("encoding").and_then(|v| v.as_str()),
        Some("base64")
    );
    let data = result
        .get("data")
        .and_then(|v| v.as_str())
        .expect("base64 data");
    use base64::{engine::general_purpose::STANDARD, Engine};
    let bytes = STANDARD.decode(data).expect("decode bundle");
    assert!(!bytes.is_empty());
}

#[test]
#[ignore = "requires live systemd/journald; run with GUESTKIT_LIVE_AGENT_TEST=1"]
fn agent_live_systemd_events_smoke() {
    if std::env::var("GUESTKIT_LIVE_AGENT_TEST").is_err() {
        return;
    }
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.getSystemdEvents","params":{"limit":5},"id":6}"#,
    );
    assert!(resp.result.is_some(), "{:?}", resp.error);
}

#[test]
fn protocol_frame_round_trip() {
    use guestkit_agent_protocol::{read_frame, write_frame};
    use std::io::Cursor;
    let payload = br#"{"jsonrpc":"2.0","method":"guestkit.ping"}"#;
    let mut buf = Vec::new();
    write_frame(&mut buf, payload).unwrap();
    let mut cursor = Cursor::new(buf);
    assert_eq!(read_frame(&mut cursor).unwrap(), payload);
}

// --- Protocol 1.3 security choke point ---

#[test]
fn file_ops_denied_by_default_policy() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.fileRead","params":{"path":"/etc/hostname"},"id":10}"#,
    );
    let err = resp.error.expect("expected policy denial");
    assert_eq!(err.code, -32005); // PolicyDenied
}

#[test]
fn expired_request_rejected() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.unsubscribeEvents","id":11,
             "ts":"2020-01-01T00:00:00Z","ttl_ms":1000}"#,
    );
    let err = resp.error.expect("expected expiry rejection");
    assert_eq!(err.code, -32003); // RequestExpired
}

#[test]
fn nonce_replay_rejected() {
    let handler = RequestHandler::new();
    let body = br#"{"jsonrpc":"2.0","method":"guestkit.unsubscribeEvents","id":12,"nonce":"replay-test-n1"}"#;
    let first = handler.handle(body);
    assert!(first.error.is_none(), "{:?}", first.error);
    let second = handler.handle(body);
    let err = second.error.expect("expected replay rejection");
    assert_eq!(err.code, -32004); // ReplayDetected
}

#[test]
fn idempotency_key_returns_cached_response() {
    let handler = RequestHandler::new();
    let first = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.unsubscribeEvents","id":13,"idempotency_key":"idem-test-k1"}"#,
    );
    assert!(first.error.is_none());
    let second = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.unsubscribeEvents","id":14,"idempotency_key":"idem-test-k1"}"#,
    );
    assert!(second.error.is_none());
    assert_eq!(second.id, Some(serde_json::json!(14)));
    assert_eq!(first.result, second.result);
}

#[test]
fn capabilities_report_categories_and_events() {
    let handler = RequestHandler::new();
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.getCapabilities","id":15}"#);
    let caps = resp.result.expect("capabilities");
    assert_eq!(caps["events"], true);
    let cats: Vec<String> =
        serde_json::from_value(caps["categories"].clone()).expect("categories array");
    assert!(cats.contains(&"telemetry".to_string()));
    assert!(!cats.contains(&"file_ops".to_string()));
}

#[test]
fn network_test_gateway_default() {
    let handler = RequestHandler::new();
    let resp =
        handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.networkTest","params":{},"id":16}"#);
    let result = resp.result.expect("network test result");
    assert!(result.get("gateway").is_some());
}

#[test]
fn performance_summary_empty_store_is_well_formed() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.getPerformanceSummary","params":{"tier":"fine"},"id":17}"#,
    );
    let result = resp.result.expect("summary");
    assert_eq!(result["tier"], "fine");
}

// --- Phase 6: enterprise automation ---

#[test]
fn packages_inventory_via_handler() {
    let handler = RequestHandler::new();
    let resp =
        handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.packages.inventory","id":30}"#);
    let result = resp.result.expect("inventory");
    assert!(result.get("installed_count").is_some());
    assert!(result.get("manager").is_some());
}

#[test]
fn packages_install_denied_by_default() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.packages.install","params":{"packages":["hello"]},"id":31}"#,
    );
    assert_eq!(resp.error.expect("denial").code, -32005);
}

#[test]
fn certificates_inventory_via_handler() {
    let handler = RequestHandler::new();
    let resp =
        handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.certificates.inventory","id":32}"#);
    let result = resp.result.expect("certs");
    assert!(result.get("certificate_count").is_some());
    assert!(result.get("ssh_host_keys").is_some());
}

#[test]
fn users_inventory_via_handler() {
    let handler = RequestHandler::new();
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.users.inventory","id":33}"#);
    assert!(resp.result.expect("users").get("user_count").is_some());
}

#[test]
fn set_hostname_denied_by_default() {
    let handler = RequestHandler::new();
    let resp = handler.handle(
        br#"{"jsonrpc":"2.0","method":"guestkit.system.setHostname","params":{"hostname":"x"},"id":34}"#,
    );
    assert_eq!(resp.error.expect("denial").code, -32005);
}

#[test]
fn phase6_dotted_aliases_resolve() {
    use guestkit_agent_protocol::RpcMethod;
    assert_eq!(
        RpcMethod::parse("packages.updates"),
        RpcMethod::PackagesUpdates
    );
    assert_eq!(
        RpcMethod::parse("certificates.inventory"),
        RpcMethod::CertificatesInventory
    );
    assert_eq!(
        RpcMethod::parse("customization.hostname"),
        RpcMethod::SetHostname
    );
}

// --- Phase 7: container awareness + offline cache (§10, §31) ---

#[test]
fn containers_inventory_via_handler() {
    let handler = RequestHandler::new();
    let resp =
        handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.containers.inventory","id":40}"#);
    let result = resp.result.expect("containers");
    assert!(result.get("runtimes").is_some());
    assert!(result.get("container_count").is_some());
    assert!(result.get("migration_risks").is_some());
}

#[test]
fn inventory_cache_offline_read_via_public_api() {
    // A cache written by a running agent is read back offline with an
    // integrity check. (write_cache's live path is exercised by the
    // real-host e2e via guestkit.inventory.cacheSnapshot.)
    let root = tempfile::tempdir().unwrap();
    let dest = root.path().join("var/lib/guestkit/inventory.snapshot");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    // Build a valid cache using the same digest the reader verifies.
    let payload = serde_json::json!({"heartbeat": {"agent_state": "healthy"}});
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(&payload).unwrap());
    let digest: String = h.finalize().iter().map(|b| format!("{b:02x}")).collect();
    let cache = serde_json::json!({
        "schema": 1, "written_at": "2026-07-19T00:00:00Z", "agent_version": "test",
        "hostname": "h", "boot_id": "b", "integrity_sha256": digest, "payload": payload
    });
    std::fs::write(&dest, serde_json::to_vec(&cache).unwrap()).unwrap();

    let read =
        guestkit::agent::inventory_cache::read_cache_from_root(root.path()).expect("offline read");
    assert_eq!(read.schema, 1);
    assert!(read.payload.get("heartbeat").is_some());
}

#[test]
fn phase7_aliases_resolve() {
    use guestkit_agent_protocol::RpcMethod;
    assert_eq!(
        RpcMethod::parse("containers.inventory"),
        RpcMethod::ContainersInventory
    );
    assert_eq!(
        RpcMethod::parse("inventory.cache"),
        RpcMethod::InventoryCacheWrite
    );
}

// --- Phase 8: tamper/integrity monitoring (§19) ---

#[test]
fn integrity_check_without_baseline() {
    let handler = RequestHandler::new();
    // Isolate state so a real baseline on the dev box doesn't interfere.
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("GUESTKIT_STATE_DIR", tmp.path());
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.integrity.check","id":50}"#);
    let result = resp.result.expect("check");
    assert_eq!(result["has_baseline"], false);
    std::env::remove_var("GUESTKIT_STATE_DIR");
}

#[test]
fn integrity_aliases_resolve() {
    use guestkit_agent_protocol::RpcMethod;
    assert_eq!(
        RpcMethod::parse("integrity.baseline"),
        RpcMethod::IntegrityBaseline
    );
    assert_eq!(
        RpcMethod::parse("security.integrity.check"),
        RpcMethod::IntegrityCheck
    );
}
