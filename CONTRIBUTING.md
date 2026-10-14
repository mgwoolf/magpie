# Contributing to Magpie

Thanks for helping out! Bug reports, ideas and pull requests are all welcome.

## Reporting bugs and ideas

Open an [issue](https://github.com/mgwoolf/magpie/issues) with:

- what you did, what you expected, and what happened instead;
- the exact text of any error from the status bar;
- your OS and version.

Please **never paste API keys** into issues or logs.

## Building

You need [Rust](https://rustup.rs). The pinned toolchain in `rust-toolchain.toml` (with `rustfmt`
and `clippy`) installs automatically.

```sh
cargo run              # debug build of the app
cargo run -- --help    # list the command-line tools
```

Linux needs a few system packages; see [Install](README.md#install) in the README.

## Before you open a pull request

CI runs these on macOS and Linux, so run them locally first:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo test -- --ignored` also runs a test that touches your real OS keychain. It's opt-in.

Some guidelines:

- Keep pull requests focused: one fix or feature each.
- Match the surrounding code style. `rustfmt.toml` sets a 120-column line width.
- Put UI styling and reusable widgets in `src/theme.rs`, so the app keeps a consistent look.
- For UI changes, add a screenshot (light and dark mode if colours changed) to the pull request.
- New image sources must have terms that allow this kind of use and must not need a paid
  account. Show each result's license in the UI.

## Developer docs

Architecture, the UI design system, recipes for common changes, egui pitfalls and the roadmap are in
[docs/](docs/README.md). AI coding agents should start with [AGENTS.md](AGENTS.md).

## Code layout

| File                | What it does                                                         |
|---------------------|----------------------------------------------------------------------|
| `src/main.rs`       | App state, the Icons/Logos/Convert tabs, toolbar, status bar and CLI |
| `src/theme.rs`      | macOS look and feel: colours, fonts, toolbar items, sidebar widgets  |
| `src/sources.rs`    | Logo and icon search (Wikidata, Simple Icons, Commons, Iconify) and HTTP helpers |
| `src/photos.rs`     | Photo providers (Openverse, Pexels, Unsplash, Pixabay)               |
| `src/photo_tab.rs`  | The Photos tab                                                       |
| `src/settings_ui.rs`| The Settings window                                                  |
| `src/convert.rs`    | SVG/raster decoding and PNG rendering                                |
| `src/cache.rs`      | On-disk cache for API responses and downloads                        |
| `src/secrets.rs`    | API keys in the OS keychain                                          |

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
