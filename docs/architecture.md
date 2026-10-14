# Architecture

Magpie is a single Rust binary (`magpie`). With no arguments it opens the GUI (eframe/egui,
glow renderer). With a subcommand it runs a CLI tool and exits (see `USAGE` in `src/main.rs`).

## Modules

| File | Responsibility |
|------|----------------|
| `src/main.rs` | Entry point and CLI; `App` state; `Settings`; the `Msg` channel; Icons/Logos tabs (`search_sidebar`, `search_tab`, `card`); Convert tab (`convert_sidebar`, `convert_ui`); toolbar, status bar and panel layout in `App::update`; settings-folder migration |
| `src/theme.rs` | The whole look and feel: palette, fonts, `Appearance`, and every custom widget. See [ui-design.md](ui-design.md) |
| `src/photo_tab.rs` | Photos tab: `PhotoState`, sidebar, grid, thumbnail workers, copy/save/credit |
| `src/settings_ui.rs` | Settings window with General and API Keys tabs; key testing |
| `src/sources.rs` | Logo search (Wikidata, Simple Icons, Commons, favicons), icon search (Iconify), and the shared HTTP layer: `client`, `send`, `describe`, `download`, `get_json_with`, `get_json_or_stale` |
| `src/photos.rs` | Photo providers (Openverse, Pexels, Unsplash, Pixabay), parallel `search`, `test_key`, download pings |
| `src/convert.rs` | SVG parsing and rendering (resvg), raster decoding (image), `preview`, `rgba`, `svg_to_png` |
| `src/cache.rs` | On-disk cache keyed by FNV-1a hash of a string key, with a TTL check on read |
| `src/secrets.rs` | API keys in the OS keychain (`keyring`), plus migration from the old service name |

## Threading model

egui is immediate mode: `App::update` runs every frame on the UI thread and must never block.

1. A UI action (e.g. **Search**) bumps a **generation** counter, clears old results, and spawns
   a `std::thread` with clones of `tx` (an `mpsc::Sender<Msg>`) and the `egui::Context`.
2. The worker does blocking work (reqwest *blocking* client, decoding, rendering), then sends a
   `Msg` tagged with the generation, and calls `ctx.request_repaint()` to wake the UI.
3. Every frame, `App::poll` drains the channel. Messages whose generation doesn't match the
   current one are **dropped**. That's how a new search cancels the old one without killing threads.

```
UI thread (update/poll)                     worker threads
-----------------------                     --------------
start_search ── spawn ───────────────────▶ sources::search / search_icons
                                             └─ tx.send(Msg::Results(gen, ..))
poll: Results(gen) ── fetch_previews ─ spawn N workers (shared job queue)
                                             └─ download → tint → convert::preview
                                             └─ tx.send(Msg::Loaded(gen, i, ..))
poll: Loaded(gen, i) → upload texture, Item.state = Ready
```

The `Msg` variants (in `src/main.rs`) are `Results`, `Loaded`, `IconSets`, `PhotoResults`,
`PhotoThumb`, `PhotoCopied`, `KeyTest` and `Status`. To add async work, add a variant, send it
from the worker, and handle it in `poll`.

Preview workers share a `Mutex<IntoIter>` job queue: 3 workers for logos (Wikimedia throttles),
8 for icons (Iconify is a CDN) and 6 for photo thumbnails.

## Search pipelines

- **Logos** (`sources::search`): Wikidata entities → their P154 logo files and official websites;
  then Simple Icons by slug; then Commons file search; then favicons of up to two official
  websites (via Google's favicon service). Results are de-duplicated by file name
  (`dedupe_key`). An error only fails the search if *every* source failed.
- **Icons** (`sources::search_icons`): Iconify search, optionally restricted to one set prefix.
  Results are grouped by set. Icons are recoloured by rewriting `currentColor` (`sources::tint`)
  before preview, save and copy (`App::output_bytes`).
- **Photos** (`photos::search`): providers run in parallel threads. Results are interleaved
  across providers. Each provider failure becomes a message, not a hard error. Openverse falls
  back to stale cached results (`get_json_or_stale`) when the API is down.

## HTTP layer (`src/sources.rs`)

- `client()`: rustls, gzip, 10 s connect timeout, 30 s total timeout, a descriptive User-Agent
  with the repo URL (Wikimedia requires a contactable UA).
- `send(build)`: up to 4 attempts. It honours `Retry-After` on 429/5xx, backs off 0.5/1/2 s on
  connection errors, and retries a timeout only once. It doesn't sleep after the last attempt.
- `describe(err)`: turns reqwest's opaque "error sending request" into the real cause
  (timeout, DNS, connection refused…). Always use it for user-facing messages.
- `get_json_with` / `download`: cached GETs (`cache::API_TTL` 7 days, `cache::FILE_TTL` 30 days).
  `download_uncached` is for full-size photos.

## Persistence

| What | Where (macOS) | How |
|------|---------------|-----|
| `Settings` (mode, colours, sizes, appearance, sidebar width…) | `~/Library/Application Support/Magpie/app.ron` | eframe `persistence` via `eframe::set_value`/`get_value` on `APP_KEY`. Saved on exit and periodically |
| API keys | Keychain, service `magpie`, account = provider name | `src/secrets.rs`. Env vars `PEXELS_API_KEY` etc. override when the stored key is empty |
| Cache | `~/Library/Caches/magpie` | `src/cache.rs`; write-then-rename for atomicity |

`Settings` is `#[serde(default)]`, so older files load fine when fields are added. `Settings.keys`
is `skip_serializing`: it's only *read* from old files so plain-text keys from early versions
can be moved into the keychain.

**Rename migrations:** the app used to be called "Logo Finder". `migrate_settings` (main.rs),
`cache::dir` and `secrets::migrate_legacy` move old data once. They can be removed after a few
releases (see [roadmap.md](roadmap.md)).

## Layout (`App::update`)

Panel order matters in egui, so panels are declared in this order:

1. `TopBottomPanel::top("toolbar")`: top margin `TITLEBAR_H` reserves the native title-bar
   strip (the window uses a full-size content view with a transparent title bar). It holds
   the window title, centred tabs and the Settings item. The whole panel is a drag region.
2. `TopBottomPanel::bottom("status")`: status text, Clear Cache, preview background, thumbnail
   size slider.
3. `SidePanel::left("sidebar")`: fixed width from `Settings.sidebar_w`, one per tab.
   `theme::sidebar_divider` draws the edge and handles resizing.
4. `CentralPanel`: the results grid or Convert preview.
5. Floating: the Settings window.

## CLI

`magpie search|icons|sets|photos|convert|clear-cache|--help` reuse the same `sources`, `photos`,
`convert` and `cache` functions as the GUI. They're the easiest way to test those modules
without the UI.
