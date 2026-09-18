# TUI Enhancements - January 2026

Carbon control-plane UI for offline disk inspect — part of [GuestKit on zyvor.dev](https://zyvor.dev/guestkit).

## Overview

Interactive terminal UI (`guestkit tui IMAGE`). Splash, vim keys, and grouped navigation.

## New Features

### 1. Splash Screen Integration ✨
- Beautiful ASCII art logo displayed on startup
- Shows "GuestKit" branding
- 800ms display duration before loading
- Smooth transition to main UI

### 2. Vim-Style Keybindings ⌨️
Now supports vim-style navigation for power users:
- `j` / `k` - Scroll down/up (same as ↑/↓)
- `g` / `G` - Jump to top/bottom (same as Home/End)
- `Ctrl-u` / `Ctrl-d` - Page up/down (same as PgUp/PgDn)

All vim bindings work alongside traditional navigation keys, so both styles are supported.

### 3. View Counts in Tabs 📊
Tabs now show item counts for better context:
- **Network (5)** - 5 network interfaces
- **Packages (1247)** - 1247 packages
- **Services (42)** - 42 services
- **Databases (3)** - 3 databases installed
- **WebServers (2)** - 2 web servers
- **Issues (12)** - 12 security issues
- **Storage (8)** - 8 mount points
- **Users (23)** - 23 user accounts
- **Kernel (156)** - 156 kernel modules

Views without counts (Dashboard, Security, Profiles) show plain names.

### 4. Updated Help System 📖
- Help overlay now documents vim keybindings
- Clearer descriptions of keyboard shortcuts
- Better organization of command categories

## Technical Details

### Files (TUI module)
- `src/cli/tui/mod.rs` — entry, event loop, fleet/refresh keys
- `src/cli/tui/ui.rs` — layout chrome, overlays, stats bar, tabs, footer
- `src/cli/tui/theme.rs` — carbon palette, themes, pane/gauge/sparkline helpers
- `src/cli/tui/widgets.rs` — chips, severity rail, progress, donut, breadcrumbs
- `src/cli/tui/views/dashboard.rs`, `views/issues.rs` — polished layouts
- `src/cli/tui/config.rs` — `theme`, `show_emoji`, `density`, `icon_mode`

### Keyboard Event Handling
Vim keybindings are context-aware and only activate when:
- Not in search mode
- Not entering a filename
- Not in other input modes

This prevents conflicts with text input.

### Color Theme (Carbon)
Control-plane palette inspired by Zellij / k9s — orange is reserved for focus and risk:
- **Background**: `#0B0E12`
- **Surface**: `#11151B` / raised `#161B22`
- **Accent (focus / high risk)**: `#FF7A00`
- **Borders (idle)**: `#2A2F38`
- **Text**: `#DCE3EA` / muted `#7D8590`
- **Semantic**: success `#3FB950`, warning `#D29922`, error `#F85149`

Themes: `carbon` (default), `high-contrast`, `minimal` — set in `[ui] theme`.

## User Experience

### Before
- No splash screen (directly to loading spinner)
- Arrow keys only for navigation
- Tabs showed only view names
- Help didn't mention vim-style controls

### After
- Polished startup with splash screen
- Vim users can use familiar j/k/g/G navigation
- Tabs show helpful counts at a glance
- Comprehensive help documentation

## Performance

- Splash screen adds only 800ms to startup
- No performance impact during normal operation
- Tab count calculation is O(1) - just reading existing data

## 2026 enhancements (implemented)

- **Progressive loading** — staged inspect with on-screen progress banner
- **Real refresh** — `r` reloads current view; `Shift+R` full re-inspect; `auto_refresh_seconds` in config
- **Command palette** — `:` for goto/export/refresh/pin; `doctor`, `migrate-plan`, `export plan`, `plan preview`, `goto assurance`
- **Assurance view** (Security group) — boot gate + migration score; `d` doctor, `t` cycle target, `e` export fix plan YAML
- **Pinned tabs** — configure `[views] pinned` in `tui.toml`; pin with palette `pin view`
- **Layout modes** — `[` / `]` cycle list / split / detail (Issues view)
- **Issue filters** — `f` cycles All / Critical / High / Medium; split detail pane with remediation
- **Compare image** — `guestkit tui disk.qcow2 --compare other.qcow2`
- **Migration handoff** — `Ctrl+M` writes `guestkit-migration-<host>.json`
- **Mouse** — click tab row to switch views (`mouse_enabled` in config)
- **Context help** — `?` view-specific; `h` full reference
- **Inspect cache flag** — `~/.cache/guestkit/<hash>/inspect.ok` after successful load

## Follow-up (latest)

- **Global search** — `Ctrl+Shift+P`, cross-view hits overlay, Enter to jump
- **Grouped jump menu** — Ctrl+P sections: Overview / System / Security
- **Files extract** — `x` downloads selected file to host cwd via `guestfs.download`
- **Compare on dashboard** — `--compare` summary in system info panel
- **Pin persists** — `pin view` writes to `~/.config/guestkit/tui.toml`
- **Layout** — `[` previous layout, `]` next
- **Palette** — Up/Down to select, Enter to run

## Fleet, cache, and layout (latest)

- **Fleet mode** — `guestkit tui vm.qcow2 --fleet ./images/` discovers `*.qcow2` etc.; sidebar; **N** / **P** switch images
- **Inspect cache** — `~/.cache/guestkit/<hash>/inspect.json` speeds repeat opens (invalidated on image mtime)
- **ASCII icons** — `icon_mode = "ascii"` in `tui.toml` for all tabs and headers
- **Packages / Services** — `[` / `]` layout modes (list / split / detail) like Issues
- **Publishing** — see [publishing.md](../development/publishing.md) for PyPI and crates.io secrets

## Visual polish (May 2026)

Shared design system and chrome refresh:

### Navigation (two-tier tabs, May 2026)

| Keys | Action |
|------|--------|
| `Tab` / `Shift+Tab` | Next/prev view in group |
| `{` / `}` | Previous/next group (Overview · System · Security) |
| `Ctrl+P` | Jump menu (filter + scroll) |
| `h` + `j`/`k` | Scroll full help |
| `,` / `.` | Scroll view tab row when pinned+group tabs overflow |
| `d` / `t` / `e` | In **Assurance**: run doctor, cycle kvm→proxmox→aws, export fix plan |
| `p` | In **Assurance** (after load): read-only fix-plan preview |
| `a` | On **Dashboard**: open Assurance |

Row 1: groups. Row 2: `★` pinned + views in active group. Compact labels: `density = "compact"` or width &lt; `auto_compact_width` (default 100). Dashboard shows **Boot: N%** when assurance is loaded.

### Design system (`theme.rs`, `widgets.rs`)
- **Chip navigation** — warm `┃ Group ┃` pills + pinned view highlights (`zyvor.dev` link color)
- **Glass mode** — Zellij-style transparency: terminal wallpaper shows through; panes use blended surfaces (`transparent = true`, `glass_opacity = 82` in `tui.toml`)
- **Chrome** — raised header/footer, accent modals, left-bar toasts, lighter dim overlay
- **Pane blocks** — muted borders; orange border/title only when focused or risk-gated
- **Stat chips** — compact Pkgs / Svcs / Users / Risk / Bookmarks row
- **List severity rail** — `▌` prefix on Issues (and reusable for other views)
- **Progress bar** — staged load: `████░░░░ 4/7 Mounting image`
- **Risk donut** — ASCII `[####····]` summary on Dashboard and Issues
- **Empty state** — placeholder panel helper for views with no data

### Chrome
- **Header v2** — health %, breadcrumb (`Issues › critical`), truncated image path, fleet/compare hints
- **Tabs** — two-row group + view selector; `▸` on active tab
- **Help** — full reference (`h`) scrolls with `j`/`k` or PgUp/PgDn; title shows line range
- **Jump menu** — viewport scroll keeps selection visible; group headers are non-selectable
- **Footer** — muted hints; `cache` when inspect cache exists; loading/fleet indicators
- **Modals** — dim layer behind palette, help, jump, global search
- **Toast** — bottom-right notice panel

### Views
- **Dashboard** — 2-column grid, theme gauges/sparklines, health gauge + risk breakdown
- **Issues** — donut summary, gauge breakdown, severity rails on findings list
- **Assurance** — doctor boot gate, migration checklist, fix-plan preview modal, palette/`:` parity with CLI

### Configuration
```toml
[ui]
theme = "carbon"           # carbon | high-contrast | minimal
show_emoji = true          # false = ASCII-only labels in chrome
icon_mode = "emoji"        # emoji | ascii
density = "comfortable"    # comfortable | compact (tab labels)
auto_compact_width = 100   # icon-only tabs below this width (unless density = compact)
transparent = true         # glass panes (needs terminal transparency enabled)
glass_opacity = 82         # 40–100, surface strength when transparent

[behavior]
default_migration_target = "kvm"
assurance_on_startup = false   # run doctor when inspect finishes
show_assurance_hint = true     # one-time palette/jump toast after load
```

## Future Enhancements

Fleet hot-reload without full guestfs teardown, wheel builds on macOS for all arch matrix legs. See [roadmap](../development/roadmap.md).

## Usage

Run the TUI with:
```bash
guestkit tui vm.qcow2
```

Or install and run:
```bash
curl -fsSL -O https://github.com/zyvorai/guestkit/releases/download/v1.2.4/guestkit-1.2.4-linux-amd64.tar.gz
guestkit tui vm.qcow2
```

## Screenshots

*(Screenshots to be added after testing with real VM images)*

## Compatibility

- Works on all terminals that support:
  - UTF-8 characters
  - 256 colors or true color
  - Crossterm backend

- Tested on:
  - Linux (primary platform)
  - Terminal emulators: gnome-terminal, alacritty, kitty, wezterm

## Configuration System ⚙️

### Overview
The TUI now supports user configuration through a TOML file located at `~/.config/guestkit/tui.toml`.

### Configuration File
Create a config file to customize your TUI experience:

```toml
[ui]
show_splash = true
splash_duration_ms = 800
show_stats_bar = true
theme = "carbon"                # carbon | high-contrast | minimal
show_emoji = true
icon_mode = "emoji"             # emoji | ascii
density = "comfortable"
mouse_enabled = true

[views]
pinned = ["dashboard", "issues", "files"]
default_layout = "split"

[behavior]
default_view = "dashboard"
auto_refresh_seconds = 0
search_case_sensitive = false
search_regex_mode = false
max_bookmarks = 20
page_scroll_lines = 10

[keybindings]
vim_mode = true
quick_jump_enabled = true
```

### Configuration Options

#### UI Settings (`[ui]`)
- **show_splash**: Enable/disable splash screen (default: `true`)
- **splash_duration_ms**: How long to show splash in milliseconds (default: `800`)
- **show_stats_bar**: Show/hide the statistics bar (default: `true`)
- **theme**: `carbon`, `high-contrast`, or `minimal` (default: `carbon`)
- **show_emoji**: Emoji in chrome labels (default: `true`; set `false` for ASCII-only)
- **icon_mode**: Tab/header icons: `emoji` or `ascii` (default: `emoji`)
- **density**: `comfortable` or `compact` tab labels (default: `comfortable`)
- **auto_compact_width**: use compact tab labels when terminal is narrower than this (default: `100`)
- **transparent**: let the terminal background show through (default: `true`; disable for opaque panels)
- **glass_opacity**: pane fill strength when transparent, 40–100 (default: `82`)
- **mouse_enabled**: Enable/disable mouse support (default: `true`)

#### View Settings (`[views]`)
- **pinned**: Tab names shown first (default: dashboard, issues, files)
- **default_layout**: `list`, `split`, or `detail` for layout-aware views

#### Behavior Settings (`[behavior]`)
- **default_view**: Which view to show on startup (default: `"dashboard"`)
  - Options: `"dashboard"`, `"network"`, `"packages"`, `"services"`, `"databases"`, `"webservers"`, `"security"`, `"issues"`, `"storage"`, `"users"`, `"kernel"`, `"profiles"`
- **auto_refresh_seconds**: Auto-refresh interval in seconds (default: `0` = disabled)
- **search_case_sensitive**: Search case-sensitive by default (default: `false`)
- **search_regex_mode**: Enable regex search by default (default: `false`)
- **max_bookmarks**: Maximum number of bookmarks (default: `20`)
- **page_scroll_lines**: Lines to scroll with Page Up/Down (default: `10`)

#### Keybindings Settings (`[keybindings]`)
- **vim_mode**: Enable vim-style navigation (default: `true`)
- **quick_jump_enabled**: Enable Ctrl+P quick jump menu (default: `true`)

### Example Configurations

**Minimal Splash, Start at Services:**
```toml
[ui]
show_splash = false

[behavior]
default_view = "services"
```

**Power User Setup:**
```toml
[ui]
show_splash = false
mouse_enabled = false

[behavior]
default_view = "issues"
search_regex_mode = true
page_scroll_lines = 20
```

**Accessibility-Focused:**
```toml
[ui]
splash_duration_ms = 2000
show_stats_bar = true

[behavior]
default_view = "dashboard"
auto_refresh_seconds = 60
```

### Configuration File Location
The configuration file is automatically loaded from:
- **Linux/macOS**: `~/.config/guestkit/tui.toml`
- **Windows**: `%APPDATA%\guestkit\tui.toml`

### Creating Configuration
1. Create the directory:
   ```bash
   mkdir -p ~/.config/guestkit
   ```

2. Create `tui.toml` from the example in this document (Configuration File section).

3. Edit to your preferences:
   ```bash
   nano ~/.config/guestkit/tui.toml
   ```

### Default Behavior
If no configuration file exists, the TUI uses built-in defaults:
- Splash screen: enabled (800ms)
- Mouse support: enabled
- Vim keybindings: enabled
- Default view: Dashboard
- Auto-refresh: disabled
- All settings match the defaults listed above

### Configuration Validation
- Invalid configuration files fall back to defaults
- Unknown fields are ignored
- Type mismatches use defaults for that setting

## Credits

- ASCII art logo and Zyvor splash branding
- Carbon palette inspired by Zellij / k9s control-plane UIs
- Vim keybindings follow standard vim conventions
- Configuration system uses TOML for human-friendly editing
