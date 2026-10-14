# Development

## Toolchain

`rust-toolchain.toml` pins Rust 1.90.0 with rustfmt and clippy; rustup installs it automatically.
Edition 2024. `rustfmt.toml` sets `max_width = 120`.

Linux needs system packages for the build (see README → Install). CI installs them in
`.github/workflows/ci.yml`.

## The loop

```sh
cargo build && ./target/debug/magpie        # run the GUI
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test                                  # unit tests (network-free)
cargo test -- --ignored                     # also the real-keychain round trip (opt-in)
```

## Tests

Unit tests live next to the code in `#[cfg(test)] mod tests`:

- `convert.rs`: SVG → PNG keeps aspect and transparency; RGBA is straight alpha; previews fit.
- `sources.rs`: Simple Icons slugs, `currentColor` tinting, Commons file URLs.
- `photos.rs`: file extensions from URLs.
- `cache.rs`: the hash is stable across Rust versions (cache keys must not change).
- `secrets.rs`: keychain round trip (`#[ignore]`d because it touches the real keychain).

Tests must not use the network. To exercise network code by hand, use the CLI:

```sh
cargo run -- search "Spotify"
cargo run -- icons "arrow" --set lucide
cargo run -- photos "mountain lake"
cargo run -- convert in.svg out.png 1024
```

There are no automated UI tests yet (see [roadmap.md](roadmap.md)).

## Verifying UI work as an agent

- Screenshots need macOS Screen Recording permission for the terminal, which agents often lack
  (`screencapture` fails with "could not create image"). Don't pretend you looked: build, launch,
  and tell the human which visual details to check.
- The agent shell may block outbound network (`curl` fails to resolve) while the built binary
  still reaches the internet. Test network paths through the `magpie` CLI.
- Relaunching: `pkill -x magpie; ./target/debug/magpie &`.

## Recipes

### Add a setting

1. Add a field to `Settings` in `src/main.rs` and set it in `impl Default`. `#[serde(default)]`
   keeps old settings files loading.
2. Add UI in `src/settings_ui.rs` (`general_settings` grid) or the relevant sidebar.
3. If it must take effect at startup, apply it in `App::new` (see `appearance.apply`).

### Add a toolbar tab

1. Add a variant to `Tab` in `src/main.rs`.
2. Add it to the `tabs` array in `App::update` with a label and a `Glyph` (add a glyph if needed;
   see [ui-design.md](ui-design.md)).
3. Add arms for it in the sidebar `match` and the `CentralPanel` `match`.
4. Raise the minimum window width (`with_min_inner_size`) if the toolbar gets crowded.

### Add a logo or icon source

1. Write `fn my_source(client: &Client, query: &str) -> Result<Vec<Candidate>, String>` in
   `src/sources.rs`, using `get_json`/`get_json_with` (cached, retried) for API calls.
2. Call it from `sources::search` in priority order, and push errors into `errors`.
3. Set `Candidate.source` to a short display name, and `kind` correctly (SVG vs PNG).
4. Document it in the README's "Where results come from" table, with its license terms.

### Add a photo provider

1. Write `fn provider(client, q, key) -> Result<Vec<Photo>, String>` in `src/photos.rs`, filling
   `license` and `credit` for every photo.
2. Register it in `photos::search` (wrapped to return `(photos, None)`), `test_key`, `Keys`,
   `secrets::PROVIDERS` and the `PROVIDERS` list in `src/settings_ui.rs`.
3. If the provider requires download pings (Unsplash does), set `Photo.download_ping`.

### Add async work

Add a `Msg` variant, spawn a thread that sends it with the current generation and calls
`ctx.request_repaint()`, and handle it in `App::poll`. See [architecture.md](architecture.md).

## Releasing

1. Bump `version` in `Cargo.toml` and move the `CHANGELOG.md` "Unreleased" notes under the new
   version.
2. Commit, tag `vX.Y.Z`, push the tag, and create a GitHub release.
3. Binaries aren't automated yet (see [roadmap.md](roadmap.md)).
