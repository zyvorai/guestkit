// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dedicated mTLS listener for guest-agent push (register, bootstrap, heartbeat, report).

use anyhow::{Context, Result};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder as ConnBuilder;
use rustls::pki_types::CertificateDer;
use rustls::server::WebPkiClientVerifier;
use rustls::RootCertStore;
use std::fs;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tower::Service;

use crate::guest_agent_ca::AgentCa;
use crate::routes;
use crate::state::AppState;

pub async fn serve(bind_addr: &str, state: AppState) -> Result<()> {
    let ca = AgentCa::from_config(state.config.agent_ca_dir.clone());
    ca.ensure_server_tls()
        .map_err(|e| anyhow::anyhow!(e.message))?;

    let server_config = build_server_config(&ca)?;
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    let app = routes::guest_agent_mtls_router().with_state(state);

    let addr: SocketAddr = bind_addr.parse().context("parse AGENT_MTLS_BIND_ADDR")?;
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("guest-agent mTLS push listening on {addr}");

    loop {
        let (tcp, peer) = listener.accept().await.context("accept guest-agent mTLS")?;
        let acceptor = acceptor.clone();
        let app = app.clone();
        tokio::spawn(async move {
            let tls = match acceptor.accept(tcp).await {
                Ok(stream) => stream,
                Err(e) => {
                    tracing::debug!("guest-agent mTLS handshake failed from {peer}: {e}");
                    return;
                }
            };
            let io = TokioIo::new(tls);
            let service = service_fn(move |req| {
                let mut app = app.clone();
                async move { app.call(req).await }
            });
            if let Err(e) = ConnBuilder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
            {
                tracing::debug!("guest-agent mTLS connection from {peer} ended: {e}");
            }
        });
    }
}

fn build_server_config(ca: &AgentCa) -> Result<rustls::ServerConfig> {
    let ca_pem = fs::read_to_string(ca.ca_cert_path()).context("read agent CA pem")?;
    let mut roots = RootCertStore::empty();
    for cert in rustls_pemfile::certs(&mut ca_pem.as_bytes()).collect::<Result<Vec<_>, _>>()? {
        roots.add(cert).context("add CA to root store")?;
    }

    let client_verifier = WebPkiClientVerifier::builder(roots.into())
        .build()
        .context("build client cert verifier")?;

    let cert_pem =
        fs::read_to_string(ca.server_cert_path()).context("read Zeus API server cert")?;
    let key_pem =
        fs::read_to_string(ca.server_key_path()).context("read Zeus API server key")?;

    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut cert_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .context("parse server cert pem")?;
    let key = rustls_pemfile::private_key(&mut key_pem.as_bytes())
        .context("parse server key pem")?
        .context("missing private key in server key pem")?;

    rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(certs, key)
        .context("build rustls server config")
}
