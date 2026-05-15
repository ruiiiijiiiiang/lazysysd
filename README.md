# lazysysd

`lazysysd` is a security-focused `ratatui` TUI application for managing `systemd` services on Linux. It prioritizes the principle of least privilege, allowing users to browse services in an unprivileged state and providing an embedded authentication flow for privileged operations.

## Features

- **Security First:** Start as an unprivileged user. `lazysysd` never needs elevated privileges itself; privileged actions are handed off to the system `polkit`, which can authenticate using whatever mechanism is available on the system, such as a password, fingerprint reader, or smart card. Password entry is handled via an integrated `pkttyagent` in an embedded PTY modal when needed.
- **Unified Unit Management:** Seamlessly browse and control both **System (global)** and **User (session)** units from a single interface.
- **Enhanced Filtering:** Powerful multi-category filters (Scope, Active, Enablement, Load).
- **Service Dashboard:** Efficiently list and discover units with high-performance client-side fuzzy search.
- **Log Viewer:** Integrated `journalctl` browser with automatic syntax highlighting. Includes a **Visual Select** mode for yanking multiple log lines to the system clipboard.
- **Unit File Editor:** View unit configurations directly with syntax highlighting. Supports creating **drop-in overrides** or editing the full unit file via your `$EDITOR`.
- **Vim-style Navigation:** Global keyboard shortcuts for intuitive scrolling, paging, and searching.
- **Asynchronous & Responsive:** Built on `tokio` and `zbus`, ensuring the UI remains ultra-smooth even during heavy D-Bus or journal operations.

<details>
  <summary>Why another TUI for managing systemd services?</summary>

This tool is not the first of its kind. I have been using [systemctl-tui](https://github.com/rgwood/systemctl-tui) and [systemd-manager-tui](https://github.com/matheus-git/systemd-manager-tui) extensively to the point that I forgot how to use `systemctl` from the command line. However those tools share one major security drawback: they require `sudo` for privileged operations. In today’s supply-chain threat landscape, that is a serious risk because a TUI app depends on many components, and any compromised dependency could become a full-privilege attack vector.

`lazysysd` was built with a different security model from the ground up. It should never be run with `sudo`; instead, privileged actions are handled through `polkit` and `pkttyagent`. The authentication flow stays fully embedded in the app and follows the principle of least privilege, keeping the experience as secure as possible.

</details>

## Keybindings

### Global

- `q`: Quit application
- `Esc`: Return to unit list / Cancel authentication / Close filter menu

### Unit List

- `j` / `k` or `Up` / `Down`: Navigate up/down
- `gg` / `G`: Jump to top/bottom
- `Ctrl+u` / `Ctrl+d`: Scroll half-page up/down
- `Ctrl+b` / `Ctrl+f`: Scroll full-page up/down
- `/`: Enter fuzzy search mode
- `p` / `a` / `n` / `o`: Open Scope, Active, Enablement, or Load filter menus
- `s` / `t` / `r` / `R`: Start, stop, restart, or reload the selected unit
- `e` / `d` / `m` / `u`: Enable, disable, mask, or unmask the selected unit
- `Enter` / `l`: View unit logs
- `v`: View unit file

### Log Viewer

- `v`: Toggle **Visual Select** mode
- `Space` (Visual Select): Toggle selection of the current line
- `y` / `Enter` (Visual Select): Yank selected lines to clipboard
- `Ctrl+r`: Refresh logs

### Unit File Viewer

- `e`: Create/Edit **drop-in override** (`override.conf`)
- `E`: Edit **full unit file** (replaces unit fragment)
- `j` / `k`: Scroll one line
- `gg` / `G`: Jump to start/end

## Technical Stack

- **UI Framework:** [ratatui](https://github.com/ratatui/ratatui)
- **Asynchronous Runtime:** [tokio](https://github.com/tokio-rs/tokio)
- **D-Bus Communication:** [zbus](https://github.com/dbus2/zbus)
- **Privilege Escalation:** `pkttyagent` managed via [portable-pty](https://github.com/wez/wezterm/tree/main/pty)
- **Highlighting:** [tailspin](https://github.com/bensadeh/tailspin) & [ansi-to-tui](https://github.com/colored-finance/ansi-to-tui)
- **Fuzzy Matching:** [fuzzy-matcher](https://github.com/lotabout/fuzzy-matcher) (SkimMatcherV2)

## Installation

### Prerequisites

- `systemd`
- `polkit`
- (Optional) a system clipboard tool: `wl-copy` (Wayland) or `xclip` (X11)

### Build with Nix

```bash
nix build
./result/bin/lazysysd
```

## License

MIT / Apache-2.0
