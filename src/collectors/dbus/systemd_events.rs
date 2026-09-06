// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live systemd D-Bus signal subscriber (VM black-box recorder).

use guestkit_agent_protocol::SystemdEvent;
use once_cell::sync::Lazy;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_EVENTS: usize = 500;
const INCIDENT_WINDOW: Duration = Duration::from_secs(240);
const INCIDENT_THRESHOLD: usize = 3;

static EVENT_BUFFER: Lazy<Mutex<SystemdEventStore>> =
    Lazy::new(|| Mutex::new(SystemdEventStore::default()));

#[derive(Debug, Default)]
struct SystemdEventStore {
    events: VecDeque<SystemdEvent>,
    failure_times: std::collections::HashMap<String, Vec<Instant>>,
    next_cursor: u64,
}

impl SystemdEventStore {
    fn push(&mut self, event: SystemdEvent) -> u64 {
        self.next_cursor += 1;
        if self.events.len() >= MAX_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.next_cursor
    }

    fn events_since(&self, cursor: u64) -> Vec<SystemdEvent> {
        if cursor >= self.next_cursor {
            return Vec::new();
        }
        let skip = self.next_cursor.saturating_sub(self.events.len() as u64);
        let start = cursor.saturating_sub(skip) as usize;
        self.events.iter().skip(start).cloned().collect()
    }

    fn recent(&self, limit: usize) -> Vec<SystemdEvent> {
        self.events.iter().rev().take(limit).cloned().collect()
    }

    fn record_failure(&mut self, unit: &str) -> bool {
        let now = Instant::now();
        let times = self.failure_times.entry(unit.to_string()).or_default();
        times.retain(|t| now.duration_since(*t) <= INCIDENT_WINDOW);
        times.push(now);
        times.len() >= INCIDENT_THRESHOLD
    }
}

pub fn push_event(kind: &str, unit: &str, detail: &str) {
    let event = SystemdEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        kind: kind.to_string(),
        unit: unit.to_string(),
        detail: detail.to_string(),
    };
    if let Ok(mut store) = EVENT_BUFFER.lock() {
        store.push(event);
    }
}

pub fn record_unit_failure(unit: &str) -> bool {
    EVENT_BUFFER
        .lock()
        .map(|mut s| s.record_failure(unit))
        .unwrap_or(false)
}

pub fn get_events_since(cursor: u64) -> (u64, Vec<SystemdEvent>) {
    EVENT_BUFFER
        .lock()
        .map(|s| (s.next_cursor, s.events_since(cursor)))
        .unwrap_or((0, Vec::new()))
}

pub fn recent_events(limit: usize) -> Vec<SystemdEvent> {
    EVENT_BUFFER
        .lock()
        .map(|s| s.recent(limit))
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
pub fn spawn_subscriber() {
    tokio::spawn(async {
        if let Err(e) = run_subscriber_async().await {
            log::warn!("systemd event subscriber stopped: {e}");
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn spawn_subscriber() {}

#[cfg(target_os = "linux")]
async fn run_subscriber_async() -> anyhow::Result<()> {
    use futures_util::StreamExt;
    use zbus::proxy;
    use zbus::zvariant::OwnedObjectPath;

    #[proxy(
        interface = "org.freedesktop.systemd1.Manager",
        default_service = "org.freedesktop.systemd1",
        default_path = "/org/freedesktop/systemd1"
    )]
    trait Manager {
        #[zbus(signal)]
        fn unit_new(&self, id: &str, unit: OwnedObjectPath) -> zbus::Result<()>;

        #[zbus(signal)]
        fn unit_removed(&self, id: &str, unit: OwnedObjectPath) -> zbus::Result<()>;

        #[zbus(signal)]
        fn job_new(&self, id: u32, job: OwnedObjectPath, unit: OwnedObjectPath)
            -> zbus::Result<()>;

        #[zbus(signal)]
        fn job_removed(
            &self,
            id: u32,
            job: OwnedObjectPath,
            unit: OwnedObjectPath,
            result: &str,
        ) -> zbus::Result<()>;
    }

    let conn = zbus::Connection::system().await?;
    let proxy = ManagerProxy::new(&conn).await?;
    let mut unit_new = proxy.receive_unit_new().await?;
    let mut unit_removed = proxy.receive_unit_removed().await?;
    let mut job_new = proxy.receive_job_new().await?;
    let mut job_removed = proxy.receive_job_removed().await?;

    loop {
        tokio::select! {
            Some(msg) = unit_new.next() => {
                if let Ok(args) = msg.args() {
                    push_event("unit_new", args.id, &format!("path={}", args.unit));
                }
            }
            Some(msg) = unit_removed.next() => {
                if let Ok(args) = msg.args() {
                    push_event("unit_removed", args.id, &format!("path={}", args.unit));
                }
            }
            Some(msg) = job_new.next() => {
                if let Ok(args) = msg.args() {
                    push_event("job_new", &format!("job-{}", args.id), &format!("unit={}", args.unit));
                }
            }
            Some(msg) = job_removed.next() => {
                if let Ok(args) = msg.args() {
                    let unit_str = args.unit.to_string();
                    push_event(
                        "job_removed",
                        &unit_str,
                        &format!("job={} result={}", args.id, args.result),
                    );
                    if (args.result == "failed" || args.result == "timeout")
                        && record_unit_failure(&unit_str) {
                            push_event(
                                "incident",
                                &unit_str,
                                "repeated service crash within 4 minutes",
                            );
                        }
                }
            }
            else => break,
        }
    }
    Ok(())
}
