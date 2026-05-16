# lazysysd

`lazysysd` is a security-focused `ratatui` TUI application for managing `systemd` services on Linux. It prioritizes the principle of least privilege, allowing users to browse services in an unprivileged state and providing an embedded authentication flow for privileged operations.

## Features

- **Security First:** Start as an unprivileged user. `lazysysd` never needs elevated privileges itself; privileged actions are handed off to the system `polkit`, which can authenticate using whatever mechanism is available on the system, such as a password, fingerprint reader, or smart card. Password entry is handled via an integrated `pkttyagent` in an embedded PTY modal when needed.
- **Unified Unit Management:** Seamlessly browse and control both **System (global)** and **User (session)** units from a single interface.
- **Enhanced Filtering:** Powerful multi-category filters (Scope, Active, Enablement, Load).
- **Service Dashboard:** Efficiently list and discover units with case-insensitive sorting and high-performance client-side fuzzy search.
- **Context Header:** The top row stays visible in every view and shows either filter state or the selected unit's live status.
- **Log Viewer:** Integrated `journalctl` browser with automatic syntax highlighting, exact search, `n/N` match cycling, and both line-wise and multi-line visual select modes.
- **Unit File Viewer:** View unit configurations directly with syntax highlighting, exact search, and `n/N` match cycling. Supports creating **drop-in overrides** or editing the full unit file via your `$EDITOR`.
- **Text Editing:** Logs and unit files open in your editor with ANSI stripped, so the buffer is plain text.
- **Vim-style Navigation:** Global keyboard shortcuts for intuitive scrolling, paging, and search cursor movement.
- **Asynchronous & Responsive:** Built on `tokio` and `zbus`, ensuring the UI remains ultra-smooth even during heavy D-Bus or journal operations.

<details>
  <summary>Why another TUI for managing systemd services?</summary>

This tool is not the first of its kind. I have been using [systemctl-tui](https://github.com/rgwood/systemctl-tui) and [systemd-manager-tui](https://github.com/matheus-git/systemd-manager-tui) extensively to the point that I forgot how to use `systemctl` from the command line. However those tools share one major security drawback: they require `sudo` for privileged operations. In today’s supply-chain threat landscape, that is a serious risk because a TUI app depends on many components, and any compromised dependency could become a full-privilege attack vector.

`lazysysd` uses a different model: the app itself never runs with `sudo`, and no action ever asks for blanket root access. When you start, stop, enable, disable, mask, unmask, reload, or edit a unit, the app opens an embedded `polkit`/`pkttyagent` flow that authenticates only the specific `systemctl` action you are trying to perform. That keeps the privilege boundary narrow, explicit, and tied to a single operation instead of the whole process.

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
- `Left` / `Right` (search): Move the search cursor
- `p` / `a` / `n` / `o`: Open Scope, Active, Enablement, or Load filter menus
- `s` / `t` / `r` / `R`: Start, stop, restart, or reload the selected unit
- `e` / `d` / `m` / `u`: Enable, disable, mask, or unmask the selected unit
- `Enter` / `l`: View unit logs
- `v`: View unit file

### Log Viewer

- `/`: Enter exact search mode
- `n` / `N`: Jump to next / previous search match
- `v`: Toggle **Visual Select** mode
- `V`: Toggle line-wise visual select mode
- `Space` (Visual Select): Toggle selection of the current line
- `y` / `Enter` (Visual Select): Yank selected lines to clipboard
- `Space` (line-wise select): Mark / unmark lines
- `Ctrl+r`: Refresh logs
- `e`: Open the log buffer in `$EDITOR`

### Unit File Viewer

- `e`: Create/Edit **drop-in override** (`override.conf`)
- `E`: Edit **full unit file** (replaces unit fragment)
- `/`: Enter exact search mode
- `n` / `N`: Jump to next / previous search match
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
