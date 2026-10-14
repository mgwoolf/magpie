# Roadmap and known gaps

Rough priority order within each section. If you pick something up, update this file.

## Before or just after the first public release

- **Screenshot** for the README (`docs/screenshot.png`; the README has a commented placeholder).
- **Confirm Linux CI.** The workflow's apt package list was inferred from the dependencies, not yet
  run. Fix it on the first CI failure.
- **Release binaries:** a GitHub Actions workflow that builds a macOS `.app` (universal binary),
  ideally signed and notarised, and attaches it to tagged releases. Linux tarball optional.
- Issue and pull-request templates in `.github/`.

## Product

- Remember the last search per tab, and put the cursor in the sidebar search field when switching
  tabs.
- Drag results out of the window into other apps (egui doesn't support drag-out yet; may need
  platform code).
- Favourites or a collection to gather assets for a project, plus export all.
- More logo sources with clear terms (e.g. official press kits via Wikidata).
- A "Magpie — Icons"-style title option, if users want the tab name in the title bar.

## UI and platform

- **Accessibility:** eframe is built with `default-features = false` and *without* `accesskit`,
  so screen readers get nothing. Enabling the `accesskit` feature and labelling the custom
  widgets (glyph-only items, search field) is the main gap.
- Bold or semibold text: needs a static SF Pro weight or variable-font support in egui.
- Real SF Symbols or native toolbar/menus would need a native (AppKit/SwiftUI) front end.
- Windows: untested. Check fonts (SF Pro is macOS-only and falls back to egui's defaults), the
  keychain backend and title-bar handling there.

## Engineering

- **UI tests** with `egui_kittest` (snapshot tests for light and dark, sidebar, cards).
- **Remove the "Logo Finder" migrations** (`migrate_settings`, `cache::dir` rename,
  `secrets::migrate_legacy`) once old installs are unlikely, e.g. after 0.3.
- Split `src/main.rs` (about 1,300 lines): move the Icons/Logos tab into `search_tab.rs` and Convert
  into `convert_tab.rs`, mirroring `photo_tab.rs`.
- Cancel in-flight downloads on a new search instead of letting stale workers finish (results are
  already discarded by generation).
- Bound the cache size (it currently only expires by age; Clear Cache empties it).
