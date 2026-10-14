# AGENTS.md

Instructions for AI coding agents (and humans) working on **Magpie**: a native Rust + egui desktop
app for finding logos, icons and stock photos, and converting SVG to PNG. It looks and behaves like
a macOS app. See [README.md](README.md) for the user-facing overview.

## Read first

1. [docs/architecture.md](docs/architecture.md): modules, threading model, data flow, persistence.
2. [docs/ui-design.md](docs/ui-design.md): the design system in `src/theme.rs`. **Read before any UI change.**
3. [docs/development.md](docs/development.md): build, test, verify, and recipes for common changes.
4. [docs/egui-pitfalls.md](docs/egui-pitfalls.md): egui behaviours that have caused bugs here.
5. [docs/decisions.md](docs/decisions.md): why things are the way they are. Check before reversing one.
6. [docs/roadmap.md](docs/roadmap.md): known gaps and next steps.

## Commands

```sh
cargo run                                   # run the app (debug)
cargo run -- --help                         # CLI tools; `cargo run -- search Tesla` etc.
cargo fmt                                   # format (120 columns, see rustfmt.toml)
cargo clippy --all-targets -- -D warnings   # lint; must be clean
cargo test                                  # unit tests; must pass
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy and tests on macOS and Linux. Run all three before
you finish a change.

## Ground rules

- **Keep the macOS look.** Use the widgets and colour helpers in `src/theme.rs`; don't hard-code
  colours or use raw `egui::Button` styling for primary actions. Check light *and* dark mode.
- **Never block the UI thread.** Network, decoding and rendering run on worker threads and report
  back through `Msg` on the channel (see architecture). Tag async results with the current
  `generation` so stale results are dropped.
- **No secrets in files.** API keys live in the OS keychain (`src/secrets.rs`) or environment
  variables. Never log, print or commit them.
- **Settings must stay backward compatible.** `Settings` is `#[serde(default)]`; add fields with a
  default, never rename or remove existing ones without a migration.
- **User-facing text:** British spelling ("Colour"), Title Case on buttons and menu items
  ("Save PNG…", "Show All Sets"), an ellipsis (…) when an action opens a dialog, and no emoji.
- **Errors are shown, not swallowed.** Network/provider failures go to the status bar with the real
  cause (see `sources::describe`).
- **Respect upstream services.** Keep the descriptive User-Agent, the on-disk cache and the
  retry/backoff in `sources::send`. Don't add sources that need paid accounts or forbid this use.
- **Update the docs** in `docs/` (and README for user-visible changes) in the same change.

## Verifying UI changes

Agent sandboxes usually can't take screenshots (macOS Screen Recording permission) and may have no
network. Build and launch the app anyway (`cargo build && ./target/debug/magpie`), use the CLI to
exercise search code paths, and **say plainly in your summary which visual details you could not
check**, so the human can look.
