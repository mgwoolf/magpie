# Decision log

Short records of choices that aren't obvious from the code. Add new entries at the bottom. If you
reverse a decision, add a new entry rather than editing the old one.

### egui, styled to look native, rather than SwiftUI or AppKit
Magpie started as a cross-platform Rust/egui tool. Restyling egui (`src/theme.rs`) got most of the
Mac feel for a fraction of the cost of a SwiftUI rewrite, and keeps Linux and Windows builds
possible. Trade-off: no real NSToolbar, SF Symbols, native menus or bold system font. A SwiftUI
front end over a Rust core is still an option (see roadmap).

### Toolbar tabs: Icons, Logos, Photos, Convert
Mac settings-style icon-over-label items centred in the toolbar. Logos and Icons were once a
segmented toggle inside one tab; they became separate tabs because they're used differently.
Convert was briefly a window behind a ⋯ menu, then came back as a tab for discoverability.

### Settings as an item in the toolbar
It's styled like the tabs and highlighted while the window is open. ⌘, also opens it. Settings
has General and API Keys tabs in the same `toolbar_tabs` style.

### A sidebar on every tab
Search and filters live in a left sidebar; results get the whole content area. All tabs share one
sidebar width (persisted).

### Custom sidebar divider
egui's resize handle turns black on hover (see [egui-pitfalls.md](egui-pitfalls.md)). We draw our
own grey hairline and drag handle.

### Custom-drawn window title
"Magpie" is drawn centred across the whole window instead of using the native title, which also
used to change per tab. The selected tab already shows where you are.

### Search field and button are custom widgets
Users found egui's default text field hard to see on the grey sidebar. `search_field` is larger,
white, outlined, with a magnifier and focus ring. The Search button carries a magnifier too.

### Preview background and sizes live in the status bar and sidebar
The toolbar is reserved for navigation. Preview background moved next to Clear Cache. PNG export
size sits in the sidebar under Search. Thumbnail size is a status-bar slider.

### Network resilience over speed
`sources::send` retries with backoff and reports the real cause via `describe`. Openverse falls
back to stale cached results because its free API is often slow or briefly down. Failed photo
searches say "Couldn't search…" rather than "No photos found".

### API keys only in the OS keychain
Never in the settings file. Env vars are supported for CLI and CI use.

### Renamed from "Logo Finder" to "Magpie"
Settings folder, cache folder and keychain service are migrated once on startup.

### MIT license
Chosen by the maintainer for maximum permissiveness. Content found through Magpie keeps its own
licenses and trademarks; the README says so.
