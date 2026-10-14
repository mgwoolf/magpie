# Magpie

[![CI](https://github.com/mgwoolf/magpie/actions/workflows/ci.yml/badge.svg)](https://github.com/mgwoolf/magpie/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Magpie is a small native desktop app, written in Rust with [egui](https://github.com/emilk/egui),
for collecting the visual bits a project needs: transparent **logos** for companies and products,
open-source **vector icons**, free **stock photos**, and quick **SVG → PNG** conversion.

No account or API key is needed to get started.

<!-- Add a screenshot at docs/screenshot.png and uncomment:
![Magpie](docs/screenshot.png)
-->

## Features

- **Logos:** search a company or product name and get transparent SVG/PNG logos from Wikidata,
  Simple Icons and Wikimedia Commons. Each result shows its source, format and whether it has a
  transparent background.
- **Icons:** search 200+ open-source icon sets (Lucide, Tabler, Phosphor, Material…) by keyword.
  Pick a colour, and lock results to one set so a project's icons stay consistent.
- **Photos:** search openly licensed photos on Openverse, plus Pexels, Unsplash and Pixabay
  if you add their free API keys. Filter by orientation and commercial-use licenses, and copy
  the credit line in one click.
- **Convert:** turn any SVG into a transparent PNG at the width you choose.
- **Copy or save:** click any result to copy it to the clipboard as a transparent PNG, or save
  the original file.
- **Mac-style interface:** toolbar tabs, a sidebar for search and filters, resizable thumbnails,
  and light, dark or automatic appearance.

## Install

Magpie is built from source for now. You need [Rust](https://rustup.rs). The pinned toolchain
(see `rust-toolchain.toml`) installs itself on the first build.

```sh
git clone https://github.com/mgwoolf/magpie.git
cd magpie
cargo run --release
```

The release binary ends up in `target/release/magpie`.

**Platforms:** Magpie is developed and tested on **macOS**. It also builds on Linux (checked in
CI), where you need these development packages first (Debian/Ubuntu names):

```sh
sudo apt install pkg-config libdbus-1-dev libxkbcommon-dev libwayland-dev \
  libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

Windows should work but is untested. Reports and fixes are welcome.

## Using Magpie

The toolbar has four tabs: **Icons**, **Logos**, **Photos** and **Convert**. Each tab has a
sidebar on the left for searching and options, and the results fill the rest of the window.

### Logos and Icons

- Type in the sidebar search box and press **Return** (or click **Search**).
- **Click a result** to copy it as a transparent PNG. **Save SVG/PNG** saves the original file,
  and **→ PNG** renders an SVG at the **PNG Export Size** set in the sidebar.
  Right-click a result for the same actions.
- **Icons only:** **Colour** tints icons in previews, saved files and copies. **Icon Set** locks
  results to one family. You can also use **Use <set> for All Icons** on any icon card.

### Photos

- Describe the photo you want, then filter by **Orientation** and **License** in the sidebar.
- **Click a photo** to copy it, **Save** to download the full-size original, and **Credit** to
  copy the attribution line. **Load more** fetches the next page.
- Add free keys for **Pexels**, **Unsplash** and **Pixabay** in **Settings → API Keys** to
  search those too.

### Convert

Open an `.svg` from the sidebar (**⌘O**), click the drop zone, or drag a file onto the window.
Set the size, then **Save PNG…** or **Copy PNG**.

### Everywhere

- **Thumbnail size:** use the slider at the bottom right, **⌘+** / **⌘−** (**⌘0** resets),
  or pinch on a trackpad. Small cards show only the image and title; the actions move to the
  right-click menu.
- **Preview background:** the dropdown next to **Clear Cache** in the status bar shows previews
  on a light checker, dark checker or any solid colour, so white logos stay visible. It doesn't
  change exported files.
- **Settings (⌘,):** appearance, PNG size, preview background and cache under **General**;
  photo API keys under **API Keys**.

On Linux and Windows, use **Ctrl** wherever this README says **⌘**.

### Command line

```sh
magpie search "Tesla"                 # list logo URLs
magpie icons "shopping cart"          # list icon URLs
magpie icons arrow --set lucide       # only from one icon set
magpie sets                           # list icon sets and their prefixes
magpie photos "team meeting"          # list stock photos
magpie convert in.svg out.png 2048    # SVG -> transparent PNG, 2048px wide
magpie clear-cache                    # delete cached searches and downloads
magpie --help                         # all commands
```

## Where results come from

| Tab    | Sources |
|--------|---------|
| Logos  | [Wikidata](https://www.wikidata.org) logo images (P154), [Simple Icons](https://simpleicons.org), [Wikimedia Commons](https://commons.wikimedia.org) file search, and the official website's favicon as a fallback |
| Icons  | [Iconify](https://iconify.design), which indexes 200+ open-source icon sets |
| Photos | [Openverse](https://openverse.org) (no key), plus [Pexels](https://www.pexels.com/api/), [Unsplash](https://unsplash.com/developers) and [Pixabay](https://pixabay.com/api/docs/) with your own free keys |

**Check the license and trademark terms before you use anything Magpie finds.** Logos are usually
trademarks of their owners, even when the image file itself is freely licensed. Icon cards show
each set's license, and photo cards show each photo's license and credit line.

## Privacy

Magpie has no server, account or analytics. It talks only to the services listed above, and only
when you search, preview or download. Favicons are fetched through Google's favicon service
(`google.com/s2/favicons`), so Google sees the website domain being looked up.

- **API keys** you type in Settings are stored in your OS keychain (macOS Keychain, Windows
  Credential Manager or the Linux Secret Service), never in a file. You can also provide them as
  `PEXELS_API_KEY`, `UNSPLASH_ACCESS_KEY` and `PIXABAY_API_KEY` environment variables.
- **Cache:** searches are cached for 7 days and downloads for 30 days, in your OS cache folder
  (`~/Library/Caches/magpie` on macOS). Clear it with **Clear Cache** or `magpie clear-cache`.
- **Settings** are saved in `~/Library/Application Support/Magpie` on macOS.

## Contributing

Bug reports, ideas and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for how
to build, test and submit changes.

## License

Magpie is released under the [MIT License](LICENSE). Content found with Magpie is covered by its
own licenses; see [Where results come from](#where-results-come-from).
