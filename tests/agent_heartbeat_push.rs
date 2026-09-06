// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Loopback test: event subscription and heartbeat push interleaved with
//! normal request/response traffic on a shared writer (requires --features agent).

#![cfg(all(feature = "agent", unix))]

use guestkit::agent::handler::RequestHandler;
use guestkit::agent::heartbeat::run_heartbeat_task;
use guestkit::agent::state::{AgentRuntime, ChannelHandle};
use guestkit::agent::transport::FramedTransport;
use guestkit_agent_protocol::{read_frame, write_frame};
use serde_json::Value;
use std::os::unix::net::UnixStream;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;

fn send_request(host: &mut UnixStream, body: &str) {
    let mut w = host.try_clone().unwrap();
    write_frame(&mut w, body.as_bytes()).unwrap();
}

fn read_json(host: &mut UnixStream) -> Value {
    let mut r = host.try_clone().unwrap();
    let frame = read_frame(&mut r).unwrap();
    serde_json::from_slice(&frame).unwrap()
}

/// Read frames until one matches `pred` or the limit is hit.
fn read_until(host: &mut UnixStream, pred: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..20 {
        let v = read_json(host);
        if pred(&v) {
            return v;
        }
    }
    panic!("expected frame not seen within 20 frames");
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_then_heartbeat_push_interleaves_with_requests() {
    let (agent_sock, host_sock) = UnixStream::pair().unwrap();
    let mut host = host_sock;
    host.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();

    let transport = FramedTransport::from_parts(
        Box::new(agent_sock.try_clone().unwrap()),
        Box::new(agent_sock),
        false,
    );
    let (mut reader, writer) = transport.split();

    let runtime = AgentRuntime::global();
    let channel = Arc::new(ChannelHandle {
        name: "loopback".to_string(),
        push_capable: true,
        subscribed: AtomicBool::new(false),
        writer: writer.clone(),
    });
    runtime.register_channel(Arc::clone(&channel));

    // Mini agent loop: same dispatch + shared-writer shape as daemon::run_channel.
    let handler = RequestHandler::with_runtime(Arc::clone(&runtime));
    let chan_for_loop = Arc::clone(&channel);
    std::thread::spawn(move || loop {
        let frame = match reader.read_message() {
            Ok(f) => f,
            Err(_) => break,
        };
        let payload = handler.handle_frame_on(&frame, Some(&chan_for_loop));
        if writer.write_message(&payload, false).is_err() {
            break;
        }
    });

    tokio::spawn(run_heartbeat_task(
        Arc::clone(&runtime),
        Duration::from_secs(1),
    ));

    // Subscribe to heartbeat events.
    send_request(
        &mut host,
        r#"{"jsonrpc":"2.0","method":"guestkit.subscribeEvents","params":{"events":["heartbeat"]},"id":1}"#,
    );
    let sub = read_until(&mut host, |v| v.get("id") == Some(&Value::from(1)));
    assert_eq!(sub["result"]["subscribed"][0], "heartbeat");

    // A pushed heartbeat notification must arrive (no id, event method).
    let hb = tokio::task::spawn_blocking({
        let mut host = host.try_clone().unwrap();
        move || {
            read_until(&mut host, |v| {
                v.get("id").is_none() && v["method"] == "guestkit.event.heartbeat"
            })
        }
    })
    .await
    .unwrap();
    assert!(hb["params"]["boot_id"].as_str().is_some());
    assert!(hb["params"]["agent_state"].as_str().is_some());

    // Request/response still works while pushes are flowing.
    send_request(
        &mut host,
        r#"{"jsonrpc":"2.0","method":"guestkit.ping","id":2}"#,
    );
    let pong = tokio::task::spawn_blocking({
        let mut host = host.try_clone().unwrap();
        move || read_until(&mut host, |v| v.get("id") == Some(&Value::from(2)))
    })
    .await
    .unwrap();
    assert_eq!(pong["result"]["pong"], true);

    // Unsubscribe stops future pushes (state flag flips immediately).
    send_request(
        &mut host,
        r#"{"jsonrpc":"2.0","method":"guestkit.unsubscribeEvents","id":3}"#,
    );
    let unsub = tokio::task::spawn_blocking({
        let mut host = host.try_clone().unwrap();
        move || read_until(&mut host, |v| v.get("id") == Some(&Value::from(3)))
    })
    .await
    .unwrap();
    assert!(unsub["result"]["subscribed"].as_array().unwrap().is_empty());
    assert!(!channel
        .subscribed
        .load(std::sync::atomic::Ordering::Relaxed));
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribe_denied_without_push_capable_channel() {
    let handler = RequestHandler::new();
    // No channel context (e.g. local unix socket): subscription refused.
    let resp = handler.handle(br#"{"jsonrpc":"2.0","method":"guestkit.subscribeEvents","id":1}"#);
    let err = resp.error.expect("expected error");
    assert_eq!(err.code, -32001); // CapabilityDenied
}
