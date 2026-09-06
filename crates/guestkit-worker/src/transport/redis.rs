// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Redis Streams job transport

use async_trait::async_trait;
use guestkit_job_spec::JobDocument;
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::error::{WorkerError, WorkerResult};
use super::JobTransport;

pub const JOBS_STREAM: &str = "zyvor:jobs";
pub const CONSUMER_GROUP: &str = "guestkit-workers";

/// Redis transport configuration
#[derive(Debug, Clone)]
pub struct RedisTransportConfig {
    pub redis_url: String,
    pub consumer_name: String,
    pub result_dir: PathBuf,
    pub block_ms: usize,
}

impl Default for RedisTransportConfig {
    fn default() -> Self {
        Self {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            consumer_name: format!("worker-{}", ulid::Ulid::new()),
            result_dir: PathBuf::from("./results"),
            block_ms: 2000,
        }
    }
}

/// Redis Streams transport for distributed job queue
pub struct RedisTransport {
    conn: ConnectionManager,
    config: RedisTransportConfig,
    pending: Arc<Mutex<HashMap<String, String>>>,
}

impl RedisTransport {
    pub async fn new(config: RedisTransportConfig) -> WorkerResult<Self> {
        let client = redis::Client::open(config.redis_url.as_str())
            .map_err(|e| WorkerError::TransportError(format!("Redis connect: {e}")))?;
        let mut conn = ConnectionManager::new(client)
            .await
            .map_err(|e| WorkerError::TransportError(format!("Redis manager: {e}")))?;

        let _: Result<(), _> = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(JOBS_STREAM)
            .arg(CONSUMER_GROUP)
            .arg("0")
            .arg("MKSTREAM")
            .query_async(&mut conn)
            .await;

        Ok(Self {
            conn,
            config,
            pending: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    async fn publish_status(&mut self, job_id: &str, status: &str, error: Option<&str>) {
        let key = format!("zyvor:job-status:{job_id}");
        let mut payload = serde_json::json!({
            "job_id": job_id,
            "status": status,
            "updated_at": chrono::Utc::now().to_rfc3339(),
        });
        if let Some(err) = error {
            payload["error"] = serde_json::Value::String(err.to_string());
        }
        let _: Result<(), _> = self
            .conn
            .set_ex::<_, _, ()>(&key, payload.to_string(), 86400)
            .await;
    }

    async fn publish_result(&mut self, job_id: &str) {
        let result_path = self.config.result_dir.join(format!("{job_id}-result.json"));
        if let Ok(content) = tokio::fs::read_to_string(&result_path).await {
            let key = format!("zyvor:results:{job_id}");
            let _: Result<(), _> = self
                .conn
                .set_ex::<_, _, ()>(&key, content, 86400)
                .await;
        }
    }
}

#[async_trait]
impl JobTransport for RedisTransport {
    async fn fetch_job(&mut self) -> WorkerResult<Option<JobDocument>> {
        let reply: redis::Value = redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg(CONSUMER_GROUP)
            .arg(&self.config.consumer_name)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(self.config.block_ms)
            .arg("STREAMS")
            .arg(JOBS_STREAM)
            .arg(">")
            .query_async(&mut self.conn)
            .await
            .map_err(|e| WorkerError::TransportError(format!("XREADGROUP: {e}")))?;

        let (stream_id, job_json) = parse_stream_reply(&reply)?;
        let Some(stream_id) = stream_id else {
            return Ok(None);
        };

        let job: JobDocument = serde_json::from_str(&job_json)
            .map_err(|e| WorkerError::TransportError(format!("Invalid job JSON: {e}")))?;

        self.pending
            .lock()
            .await
            .insert(job.job_id.clone(), stream_id.clone());

        self.publish_status(&job.job_id, "running", None).await;
        Ok(Some(job))
    }

    async fn ack_job(&mut self, job_id: &str) -> WorkerResult<()> {
        let stream_id = self.pending.lock().await.remove(job_id);
        if let Some(stream_id) = stream_id {
            let _: i64 = redis::cmd("XACK")
                .arg(JOBS_STREAM)
                .arg(CONSUMER_GROUP)
                .arg(&stream_id)
                .query_async(&mut self.conn)
                .await
                .map_err(|e| WorkerError::TransportError(format!("XACK: {e}")))?;
        }
        self.publish_result(job_id).await;
        self.publish_status(job_id, "completed", None).await;
        Ok(())
    }

    async fn nack_job(&mut self, job_id: &str, reason: &str) -> WorkerResult<()> {
        let stream_id = self.pending.lock().await.remove(job_id);
        if let Some(stream_id) = stream_id {
            let _: i64 = redis::cmd("XACK")
                .arg(JOBS_STREAM)
                .arg(CONSUMER_GROUP)
                .arg(&stream_id)
                .query_async(&mut self.conn)
                .await
                .map_err(|e| WorkerError::TransportError(format!("XACK: {e}")))?;
        }
        self.publish_status(job_id, "failed", Some(reason)).await;
        Ok(())
    }

    async fn health_check(&self) -> WorkerResult<bool> {
        let client = redis::Client::open(self.config.redis_url.as_str())
            .map_err(|e| WorkerError::TransportError(e.to_string()))?;
        let mut conn = ConnectionManager::new(client)
            .await
            .map_err(|e| WorkerError::TransportError(e.to_string()))?;
        let pong: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| WorkerError::TransportError(e.to_string()))?;
        Ok(pong == "PONG" || pong == "pong")
    }
}

fn parse_stream_reply(reply: &redis::Value) -> WorkerResult<(Option<String>, String)> {
    let Some(streams) = value_as_array(reply) else {
        return Ok((None, String::new()));
    };
    if streams.is_empty() {
        return Ok((None, String::new()));
    }
    let Some(stream_entry) = value_as_array(&streams[0]) else {
        return Ok((None, String::new()));
    };
    if stream_entry.len() < 2 {
        return Ok((None, String::new()));
    }
    let Some(messages) = value_as_array(&stream_entry[1]) else {
        return Ok((None, String::new()));
    };
    if messages.is_empty() {
        return Ok((None, String::new()));
    }
    let Some(message) = value_as_array(&messages[0]) else {
        return Ok((None, String::new()));
    };
    if message.len() < 2 {
        return Ok((None, String::new()));
    }
    let stream_id = value_to_string(&message[0]);
    if stream_id.is_empty() {
        return Ok((None, String::new()));
    }
    let Some(fields) = value_as_array(&message[1]) else {
        return Ok((Some(stream_id), String::new()));
    };
    for chunk in fields.chunks(2) {
        if chunk.len() == 2 {
            let key = value_to_string(&chunk[0]);
            if key == "job" {
                return Ok((Some(stream_id), value_to_string(&chunk[1])));
            }
        }
    }
    Ok((Some(stream_id), String::new()))
}

fn value_as_array(value: &redis::Value) -> Option<&Vec<redis::Value>> {
    match value {
        redis::Value::Array(items) => Some(items),
        _ => None,
    }
}

fn value_to_string(value: &redis::Value) -> String {
    match value {
        redis::Value::BulkString(bytes) => String::from_utf8_lossy(bytes).to_string(),
        redis::Value::SimpleString(s) => s.clone(),
        redis::Value::Int(i) => i.to_string(),
        _ => String::new(),
    }
}

/// Enqueue a job document to the Redis stream (used by zyvor-api).
pub async fn enqueue_job(redis_url: &str, job: &JobDocument) -> WorkerResult<String> {
    let client = redis::Client::open(redis_url)
        .map_err(|e| WorkerError::TransportError(format!("Redis connect: {e}")))?;
    let mut conn = ConnectionManager::new(client)
        .await
        .map_err(|e| WorkerError::TransportError(format!("Redis manager: {e}")))?;

    let job_json = serde_json::to_string(job)
        .map_err(|e| WorkerError::TransportError(e.to_string()))?;

    let _: String = redis::cmd("XADD")
        .arg(JOBS_STREAM)
        .arg("*")
        .arg("job")
        .arg(job_json)
        .query_async(&mut conn)
        .await
        .map_err(|e| WorkerError::TransportError(format!("XADD: {e}")))?;

    let status_key = format!("zyvor:job-status:{}", job.job_id);
    let status = serde_json::json!({
        "job_id": job.job_id,
        "status": "pending",
        "updated_at": chrono::Utc::now().to_rfc3339(),
    });
    let _: () = conn
        .set_ex(&status_key, status.to_string(), 86400)
        .await
        .map_err(|e| WorkerError::TransportError(e.to_string()))?;

    Ok(job.job_id.clone())
}
