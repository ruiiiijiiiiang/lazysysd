# lazysysd

`lazysysd` is a security-focused `ratatui` TUI application for managing `systemd` services on Linux. It prioritizes the principle of least privilege, allowing users to browse services in an unprivileged state and providing an embedded authentication flow for privileged operations.

## Features

- **Security First:** Start as an unprivileged user. Use `polkit` for interactive authorization only when needed, handling password entry via an integrated `pkttyagent` in an embedded PTY modal.
- **Service Dashboard:** Efficiently list and filter all systemd units with high-performance client-side fuzzy search.
- **Log Viewer:** Integrated `journalctl` browser with automatic syntax highlighting (dates, IPs, log levels) powered by `tailspin`.
- **Unit File Editor:** View unit configurations directly in the TUI and suspend to your `$EDITOR` for quick modifications.
- **Vim-style Navigation:** Global keyboard shortcuts for scrolling, paging, and searching.
- **Asynchronous & Responsive:** Built on `tokio` and `zbus`, ensuring the UI remains ultra-smooth even during heavy D-Bus or journal operations.

## Keybindings

### Global

- `q`: Quit application
- `r`: Refresh unit list or current view
- `Esc`: Return to unit list / Cancel authentication

### Unit List

- `j` / `k`: Navigate up/down
- `/`: Enter fuzzy search mode
- `Enter` / `l`: View unit logs
- `v`: View unit file
- `a`: Restart unit (triggers authentication modal if needed)

### Log / File Viewer

- `j` / `k`: Scroll one line
- `Ctrl+u` / `Ctrl+d`: Scroll half-page up/down
- `Ctrl+b` / `Ctrl+f`: Scroll full-page up/down
- `e` (File View only): Edit file in `$EDITOR`

## Technical Stack

- **UI Framework:** [ratatui](https://github.com/ratatui/ratatui)
- **Asynchronous Runtime:** [tokio](https://github.com/tokio-rs/tokio)
- **D-Bus Communication:** [zbus](https://github.com/dbus2/zbus)
- **Privilege Escalation:** `pkttyagent` managed via [portable-pty](https://github.com/wez/wezterm/tree/main/pty)
- **Highlighting:** [tailspin](https://github.com/bensadeh/tailspin) & [ansi-to-tui](https://github.com/colored-finance/ansi-to-tui)
- **Fuzzy Matching:** [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher)

## Installation

### Prerequisites

- `systemd`
- `polkit`
- Rust toolchain (latest stable)

### Build from source

```bash
git clone https://github.com/youruser/lazysysd.git
cd lazysysd
cargo build --release
./target/release/lazysysd
```

## Architecture

The project follows a modular structure for better maintainability:

- `src/main.rs`: Entry point and async event loop.
- `src/app/state.rs`: Central application state and input handling.
- `src/systemd/`: Specialized logic for D-Bus (`dbus.rs`), authentication (`auth.rs`), and journal logging (`journal.rs`).
- `src/ui/`: Rendering logic (`render.rs`) and terminal utilities (`utils.rs`).
- `src/models.rs`: Shared domain models and internal event types.

## License

MIT / Apache-2.0
