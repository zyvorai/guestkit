// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live collectors for in-guest agent.

#[cfg(feature = "agent")]
pub mod dbus;

#[cfg(feature = "agent")]
pub mod hardware;

#[cfg(feature = "agent")]
pub mod network_live;

#[cfg(feature = "agent")]
pub mod process;

#[cfg(feature = "agent")]
pub mod pressure;

#[cfg(feature = "agent")]
pub mod windows_live;
