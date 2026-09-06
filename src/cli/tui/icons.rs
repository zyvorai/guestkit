// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Tab and header icons (emoji or ASCII).

use super::app::View;

pub fn view_icon(view: View, ascii: bool) -> &'static str {
    if ascii {
        match view {
            View::Dashboard => "[=] ",
            View::Analytics => "[A] ",
            View::Timeline => "[T] ",
            View::Recommendations => "[R] ",
            View::Topology => "[^] ",
            View::Network => "[N] ",
            View::Packages => "[P] ",
            View::Services => "[S] ",
            View::Databases => "[D] ",
            View::WebServers => "[W] ",
            View::Security => "[X] ",
            View::Issues => "[!] ",
            View::Storage => "[#] ",
            View::Users => "[U] ",
            View::Kernel => "[K] ",
            View::Logs => "[L] ",
            View::Profiles => "[F] ",
            View::Assurance => "[M] ",
            View::SystemdDeep => "[U] ",
            View::AiInsights => "[A] ",
            View::Files => "[>] ",
        }
    } else {
        match view {
            View::Dashboard => "📊 ",
            View::Analytics => "📈 ",
            View::Timeline => "⏰ ",
            View::Recommendations => "💡 ",
            View::Topology => "🏗️ ",
            View::Network => "🌐 ",
            View::Packages => "📦 ",
            View::Services => "⚙️ ",
            View::Databases => "🗄️ ",
            View::WebServers => "🌐 ",
            View::Security => "🔒 ",
            View::Issues => "⚠️ ",
            View::Storage => "💾 ",
            View::Users => "👥 ",
            View::Kernel => "🧩 ",
            View::Logs => "📋 ",
            View::Profiles => "🛡️ ",
            View::Assurance => "🩺 ",
            View::SystemdDeep => "🧩 ",
            View::AiInsights => "🤖 ",
            View::Files => "📂 ",
        }
    }
}

pub fn view_description(view: View) -> &'static str {
    match view {
        View::Dashboard => "System Overview",
        View::Analytics => "Analytics & Charts",
        View::Timeline => "System Timeline",
        View::Recommendations => "Smart Recommendations",
        View::Topology => "System Topology",
        View::Network => "Network Configuration",
        View::Packages => "Installed Packages",
        View::Services => "System Services",
        View::Databases => "Database Installations",
        View::WebServers => "Web Server Installations",
        View::Security => "Security Features",
        View::Issues => "Security Issues & Findings",
        View::Storage => "Storage & Filesystems",
        View::Users => "User Accounts",
        View::Kernel => "Kernel Configuration",
        View::Logs => "System Logs",
        View::Profiles => "Profile Reports",
        View::Assurance => "Boot & migration assurance",
        View::SystemdDeep => "Systemd deep dive (timers, problems, sandbox)",
        View::AiInsights => "Guest Intelligence recommendations",
        View::Files => "File Browser",
    }
}
