// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Stdio transport for development and testing.

use super::FramedTransport;
use anyhow::Result;
use std::io::{stdin, stdout};

pub fn open() -> Result<FramedTransport> {
    Ok(FramedTransport {
        reader: Box::new(stdin()),
        writer: Box::new(stdout()),
        line_framing: false,
    })
}
