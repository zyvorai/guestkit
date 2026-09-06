// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Interactive shell for VM inspection

pub mod commands;
pub mod completion;
pub mod explore;
pub mod repl;

pub use repl::run_interactive_shell;
