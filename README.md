# lazysysd

`lazysysd` is a security-focused `ratatui` TUI application for managing `systemd` services on Linux. It prioritizes the principle of least privilege, allowing users to browse services in an unprivileged state and providing an embedded authentication flow for privileged operations.

## Features

- **Unified Unit Management:** Seamlessly browse and control both **System (global)** and **User (session)** units from a single interface.
- **Enhanced Filtering:** Powerful multi-category filters (Scope, Active, Enablement, Load).
- **Service Dashboard:** Efficiently list and discover units with case-insensitive sorting and high-performance client-side fuzzy search.
- **Log Viewer:** Integrated `journalctl` browser with automatic syntax highlighting provided by [tailspin](https://github.com/bensadeh/tailspin), and both line-wise and multi-line visual select modes.
- **Unit File Viewer:** View unit configurations directly with syntax highlighting. Supports creating **drop-in overrides** or editing the full unit file via your `$EDITOR`.
- **Vim-style Navigation:** Global keyboard shortcuts for intuitive scrolling, paging, and search cursor movement.

<details>
  <summary>Why another TUI for managing systemd services?</summary>

This tool is not the first of its kind. I have been using [`systemctl-tui`](https://github.com/rgwood/systemctl-tui) and [`systemd-manager-tui`](https://github.com/matheus-git/systemd-manager-tui) extensively to the point that I forgot how to use `systemctl` from the command line. However those tools share one major limitation: they require `sudo` for privileged operations. In today’s supply-chain threat landscape, that is a serious risk because a TUI app depends on many components, and any compromised dependency could become a full-privilege attack vector.

This is why I built `lazysysd` with a completely different security model: the app itself should never be run with `sudo`, and no action ever asks for blanket root access. When you start, stop, enable, disable, mask, unmask, reload, or edit a unit, the app opens an embedded `polkit`/`pkttyagent` flow that authenticates only the specific `systemctl` action you are trying to perform, using whatever mechanism is available on the system, such as password, fingerprint reader, or smart card. That keeps the privilege boundary explicit and tied to a single operation instead of the whole process.

</details>

## Keybindings

### Global

- `q`: Quit application
- `Esc`: Return to unit list / Cancel authentication / Close filter menu
- `j` / `k` or `Up` / `Down`: Navigate up/down
- `gg` / `G`: Jump to top/bottom
- `Ctrl+u` / `Ctrl+d`: Scroll half-page up/down
- `Ctrl+b` / `Ctrl+f`: Scroll full-page up/down

### Unit List

- `/`: Enter fuzzy search mode
- `p` / `a` / `n` / `o`: Open Scope, Active, Enablement, or Load filter menus
- `Ctrl+r`: Reset all filters
- `s` / `t` / `r` / `R`: Start, stop, restart, or reload the selected unit
- `e` / `d` / `m` / `u`: Enable, disable, mask, or unmask the selected unit
- `Enter` / `l`: View unit logs
- `f`: View unit file
- `Y`: Yank unit file path

### Log Viewer

- `/`: Enter search mode
- `n` / `N`: Jump to next / previous search match
- `v`: Toggle **Visual Select** mode
  - `Space` (Visual Select): Toggle selection of the current line
- `V`: Toggle line-wise visual select mode
  - `Space` (line-wise select): Mark / unmark lines
- `y` / `Enter` (Visual Select): Yank selected lines to clipboard
- `Ctrl+r`: Refresh logs
- `e`: Open the log buffer in `$EDITOR`

### Unit File Viewer

- `/`: Enter search mode
- `n` / `N`: Jump to next / previous search match
- `e`: Create/Edit **drop-in override** (`override.conf`)
- `E`: Edit **full unit file** (replaces unit fragment)

## Technical Stack

- **UI Framework:** [ratatui](https://github.com/ratatui/ratatui)
- **Asynchronous Runtime:** [tokio](https://github.com/tokio-rs/tokio)
- **D-Bus Communication:** [zbus](https://github.com/dbus2/zbus)
- **Privilege Escalation:** `pkttyagent` managed via [portable-pty](https://github.com/wez/wezterm/tree/main/pty)
- **Highlighting:** [tailspin](https://github.com/bensadeh/tailspin) & [ansi-to-tui](https://github.com/ratatui/ansi-to-tui)
- **Fuzzy Matching:** [nucleo](https://github.com/helix-editor/nucleo)

## Installation

### Prerequisites

- `systemd`
- `polkit`
- (Optional) terminal text editor: `nano`, `vim`, `vi`, or `emacs`
- (Optional) system clipboard tool: `wl-copy` (Wayland) or `xclip` (X11)

### Build with Nix

```bash
nix build
./result/bin/lazysysd
```

## Contribution

PR's and issues are welcome. AI usage is okay as long as you know what you are doing and the code is maintainable.

## License

MIT
