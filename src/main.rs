mod cache;
mod convert;
mod photo_tab;
mod photos;
mod secrets;
mod settings_ui;
mod sources;
mod theme;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, ColorImage, Rect, Sense, TextureHandle, TextureOptions, pos2, vec2};

use convert::Raster;
use photo_tab::PhotoState;
use photos::{Orientation, Photo};
use serde::{Deserialize, Serialize};
use sources::{Candidate, IconSet, Kind};

const APP_NAME: &str = "Magpie";
const USAGE: &str = "\
usage:
  magpie                                     open the app
  magpie search <name>                       list logo URLs
  magpie icons <keyword> [--set <prefix>]    list vector icon URLs, optionally from one set
  magpie sets                                list icon sets and their prefixes
  magpie photos <description>                list stock photos (Openverse + keyed providers)
  magpie convert <in.svg> <out.png> [width]  convert SVG to transparent PNG
  magpie clear-cache                         delete cached searches and downloads
  magpie --help                              show this help";

/// Large enough to stay sharp on Retina at the biggest grid size.
const PREVIEW_PX: u32 = 512;
const DOWNLOAD_WORKERS: usize = 3;
/// Result card width range (the grid size slider) and its default.
pub(crate) const CARD_W_MIN: f32 = 120.0;
pub(crate) const CARD_W_MAX: f32 = 400.0;
pub(crate) const CARD_W: f32 = 210.0;
/// Sidebar width range and default.
const SIDEBAR_W_MIN: f32 = 200.0;
const SIDEBAR_W_MAX: f32 = 360.0;
const SIDEBAR_W: f32 = 240.0;
/// Below this card width only the thumbnail and title are shown; actions move to the right-click menu.
pub(crate) const COMPACT_W: f32 = 180.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("convert") {
        return cli_convert(&args[1..]);
    }
    if args.first().map(String::as_str) == Some("search") {
        return cli_search(&args[1..], false);
    }
    if args.first().map(String::as_str) == Some("clear-cache") {
        println!("freed {}", human_size(cache::size()));
        return cache::clear().map_err(Into::into);
    }
    if args.first().map(String::as_str) == Some("photos") {
        let query = photos::Query {
            text: args[1..].join(" "),
            page: 1,
            orientation: Orientation::Any,
            commercial_only: true,
        };
        // Uses PEXELS_API_KEY / UNSPLASH_ACCESS_KEY / PIXABAY_API_KEY if set.
        let (found, errors) = photos::search(&sources::client(), &query, &photos::Keys::default().resolved());
        for p in found {
            println!(
                "[{}] {} — {} ({}×{}, {})\n      {}",
                p.provider, p.title, p.author, p.width, p.height, p.license, p.full_url
            );
        }
        for e in errors {
            eprintln!("warning: {e}");
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("sets") {
        for set in sources::icon_sets(&sources::client())? {
            println!(
                "{:<28} {:<40} {:>6} icons  {}",
                set.prefix, set.name, set.total, set.license
            );
        }
        return Ok(());
    }
    if args.first().map(String::as_str) == Some("icons") {
        return cli_search(&args[1..], true);
    }
    if !args.is_empty() {
        let help = matches!(args[0].as_str(), "-h" | "--help" | "help");
        if help {
            println!("{USAGE}");
            return Ok(());
        }
        eprintln!("{USAGE}");
        std::process::exit(2);
    }

    migrate_settings();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([960.0, 680.0])
            .with_min_inner_size([720.0, 400.0])
            .with_drag_and_drop(true)
            // Unified title bar: content runs under it and the toolbar sits beside the traffic lights.
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            // Hidden: theme::window_title draws "Magpie" centred across the whole window instead.
            .with_title_shown(false),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            // ⌘+ / ⌘− resize the result grid instead of zooming the whole UI.
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            let settings = cc
                .storage
                .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
                .unwrap_or_default();
            Ok(Box::new(App::new(cc.egui_ctx.clone(), settings)))
        }),
    )?;
    Ok(())
}

/// The app was called "Logo Finder" before; move its saved settings to Magpie's folder once.
fn migrate_settings() {
    let (Some(old), Some(new)) = (eframe::storage_dir("Logo Finder"), eframe::storage_dir(APP_NAME)) else {
        return;
    };
    if old.exists() && !new.exists() {
        let _ = std::fs::rename(old, new);
    }
}

fn cli_search(args: &[String], icons: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut words = Vec::new();
    let mut set = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--set" {
            set = it.next().cloned();
        } else {
            words.push(a.as_str());
        }
    }
    let query = &words.join(" ");
    let client = sources::client();
    let found = if icons {
        sources::search_icons(&client, query, set.as_deref())?
    } else {
        sources::search(&client, query)?
    };
    for c in found {
        println!("[{}] {:<12} {}\n      {}", c.kind.ext(), c.source, c.title, c.url);
    }
    Ok(())
}

fn cli_convert(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (Some(input), Some(output)) = (args.first(), args.get(1)) else {
        return Err("usage: magpie convert <in.svg> <out.png> [width]".into());
    };
    let width: u32 = args.get(2).map(|w| w.parse()).transpose()?.unwrap_or(1024);
    let png = convert::svg_to_png(&std::fs::read(input)?, width)?;
    std::fs::write(output, png)?;
    println!("wrote {output} ({width}px wide)");
    Ok(())
}

enum Msg {
    Results(u64, Result<Vec<Candidate>, String>),
    Loaded(u64, usize, Result<(Vec<u8>, Raster), String>),
    IconSets(Result<Vec<IconSet>, String>),
    PhotoResults(u64, Vec<Photo>, Vec<String>),
    PhotoThumb(u64, usize, Result<Raster, String>),
    /// Decoded RGBA photo plus its credit line, ready for the clipboard.
    PhotoCopied(Result<(u32, u32, Vec<u8>, String), String>),
    KeyTest(&'static str, Result<usize, String>),
    Status(String),
}

struct Item {
    cand: Candidate,
    state: Load,
}

enum Load {
    Pending,
    Failed(String),
    Ready {
        bytes: Vec<u8>,
        tex: TextureHandle,
        transparent: bool,
    },
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum Mode {
    Logos,
    Icons,
}

/// What the logo previews are drawn on top of.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
enum Background {
    LightChecker,
    DarkChecker,
    Solid([u8; 3]),
}

impl Background {
    fn label(self) -> &'static str {
        match self {
            Background::LightChecker => "Light checker",
            Background::DarkChecker => "Dark checker",
            Background::Solid(_) => "Solid colour",
        }
    }
}

/// Preferences remembered between launches.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    mode: Mode,
    icon_color: [u8; 3],
    /// Locked icon family (Iconify prefix), so a project's icons stay consistent.
    icon_set: Option<String>,
    background: Background,
    png_width: u32,
    /// Lives in the OS keychain. Still *read* from old settings files so plain-text
    /// keys from earlier versions can be moved into the keychain, but never written.
    #[serde(skip_serializing)]
    keys: photos::Keys,
    photo_orientation: Orientation,
    commercial_only: bool,
    /// Result card width, set with the grid size slider.
    card_w: f32,
    appearance: theme::Appearance,
    /// Sidebar width, dragged by its right edge.
    sidebar_w: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Logos,
            icon_color: [0, 0, 0],
            icon_set: None,
            background: Background::LightChecker,
            png_width: 1024,
            keys: photos::Keys::default(),
            photo_orientation: Orientation::Any,
            commercial_only: true,
            card_w: CARD_W,
            appearance: theme::Appearance::Auto,
            sidebar_w: SIDEBAR_W,
        }
    }
}

/// A pending save from a result card: file stem, extension and the bytes (or why they failed).
type SaveAction = (String, &'static str, Result<Vec<u8>, String>);

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Icons,
    Logos,
    Photos,
    Convert,
}

#[derive(Clone, Copy, PartialEq)]
enum SettingsTab {
    General,
    ApiKeys,
}

struct ConvertFile {
    path: PathBuf,
    bytes: Vec<u8>,
    tex: TextureHandle,
}

struct App {
    ctx: egui::Context,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    tab: Tab,
    settings: Settings,
    /// The icon colour the previews were last rendered in.
    rendered_color: [u8; 3],
    icon_sets: Vec<IconSet>,
    icon_sets_requested: bool,
    set_filter: String,
    query: String,
    last_query: String,
    generation: u64,
    searching: bool,
    items: Vec<Item>,
    status: String,
    convert: Option<ConvertFile>,
    photos: PhotoState,
    show_settings: bool,
    settings_tab: SettingsTab,
    reveal_keys: bool,
    /// Per-provider key test: `None` while running.
    key_tests: HashMap<&'static str, Option<Result<usize, String>>>,
    /// Keys edited in Settings but not yet written to the keychain.
    keys_dirty: bool,
    clipboard: Option<arboard::Clipboard>,
}

impl App {
    fn new(ctx: egui::Context, mut settings: Settings) -> Self {
        let (tx, rx) = channel();
        settings.appearance.apply(&ctx);
        let (stored, errors) = secrets::load();
        let legacy = std::mem::take(&mut settings.keys);
        let mut status = None;
        if !errors.is_empty() {
            status = Some(format!(
                "Could not read API keys from the keychain: {}",
                errors.join("; ")
            ));
        }
        // Migrate plain-text keys saved by earlier versions into the keychain.
        let mut keys = stored;
        let mut migrated = false;
        for (slot, old) in [
            (&mut keys.pexels, legacy.pexels),
            (&mut keys.unsplash, legacy.unsplash),
            (&mut keys.pixabay, legacy.pixabay),
        ] {
            if slot.is_empty() && !old.trim().is_empty() {
                *slot = old;
                migrated = true;
            }
        }
        if migrated {
            let failed = secrets::save(&keys);
            status = Some(if failed.is_empty() {
                "Moved your API keys out of the settings file into the Keychain.".into()
            } else {
                format!("Could not move API keys into the keychain: {}", failed.join("; "))
            });
        }
        settings.keys = keys;
        let tab = match settings.mode {
            Mode::Icons => Tab::Icons,
            Mode::Logos => Tab::Logos,
        };
        Self {
            ctx,
            tx,
            rx,
            tab,
            rendered_color: settings.icon_color,
            settings,
            icon_sets: Vec::new(),
            icon_sets_requested: false,
            set_filter: String::new(),
            query: String::new(),
            last_query: String::new(),
            generation: 0,
            searching: false,
            items: Vec::new(),
            status: status.unwrap_or_else(|| {
                "Enter a company or product name, e.g. \"Spotify\" or \"Visual Studio Code\".".into()
            }),
            convert: None,
            photos: PhotoState::default(),
            show_settings: false,
            settings_tab: SettingsTab::General,
            reveal_keys: false,
            key_tests: HashMap::new(),
            keys_dirty: false,
            // Kept alive for the app's lifetime; on Linux the clipboard contents
            // disappear when the owner is dropped.
            clipboard: arboard::Clipboard::new().ok(),
        }
    }

    pub(crate) fn card_w(&self) -> f32 {
        self.settings.card_w.clamp(CARD_W_MIN, CARD_W_MAX)
    }

    /// Thumbnail size inside a card: the card's content width at a 19:13 aspect.
    pub(crate) fn thumb_size(&self) -> egui::Vec2 {
        let w = self.card_w() - 18.0;
        vec2(w, (w * 13.0 / 19.0).round())
    }

    /// ⌘+ / ⌘− / ⌘0 and trackpad pinch resize the result grid.
    fn grid_zoom(&mut self, ctx: &egui::Context) {
        let (bigger, smaller, reset, pinch) = ctx.input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Plus)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Equals),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Minus),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Num0),
                i.zoom_delta(),
            )
        });
        let w = &mut self.settings.card_w;
        if bigger {
            *w += 30.0;
        }
        if smaller {
            *w -= 30.0;
        }
        if reset {
            *w = CARD_W;
        }
        if pinch != 1.0 {
            *w *= pinch;
        }
        *w = w.clamp(CARD_W_MIN, CARD_W_MAX);
    }

    fn start_search(&mut self) {
        let query = self.query.trim().to_string();
        if query.is_empty() {
            return;
        }
        self.generation += 1;
        self.searching = true;
        self.items.clear();
        self.last_query = query.clone();
        self.status = format!("Searching for \"{query}\"…");

        let (tx, ctx, generation, mode) = (self.tx.clone(), self.ctx.clone(), self.generation, self.settings.mode);
        let set = self.settings.icon_set.clone();
        thread::spawn(move || {
            let client = sources::client();
            let res = match mode {
                Mode::Logos => sources::search(&client, &query),
                Mode::Icons => sources::search_icons(&client, &query, set.as_deref()),
            };
            let _ = tx.send(Msg::Results(generation, res));
            ctx.request_repaint();
        });
    }

    /// Download every candidate on a few worker threads (Wikimedia throttles
    /// clients that fire off many requests at once).
    fn fetch_previews(&self) {
        let client = sources::client();
        let jobs: Vec<(usize, Candidate)> = self.items.iter().map(|it| it.cand.clone()).enumerate().collect();
        let queue = Arc::new(Mutex::new(jobs.into_iter()));
        let workers = match self.settings.mode {
            Mode::Logos => DOWNLOAD_WORKERS,
            Mode::Icons => 8, // Iconify is a CDN and doesn't throttle like Wikimedia.
        };
        let hex = self.icon_hex();
        for _ in 0..workers {
            let (tx, ctx, generation, hex) = (self.tx.clone(), self.ctx.clone(), self.generation, hex.clone());
            let (client, queue) = (client.clone(), queue.clone());
            thread::spawn(move || {
                loop {
                    let Some((i, cand)) = queue.lock().unwrap().next() else {
                        break;
                    };
                    let res = sources::download(&client, &cand.url).and_then(|bytes| {
                        let shown = if cand.tintable() {
                            sources::tint(&bytes, &hex)
                        } else {
                            bytes.clone()
                        };
                        let raster = convert::preview(&shown, cand.kind == Kind::Svg, PREVIEW_PX)?;
                        Ok((bytes, raster))
                    });
                    if tx.send(Msg::Loaded(generation, i, res)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Results(g, res) if g == self.generation => {
                    self.searching = false;
                    match res {
                        Ok(cands) if cands.is_empty() => {
                            self.status = match (self.settings.mode, self.locked_set_name()) {
                                (Mode::Icons, Some(set)) => format!(
                                    "No \"{}\" icons in {set}. Try another keyword, or unlock the icon set.",
                                    self.last_query
                                ),
                                (Mode::Icons, None) => format!("No icons found for \"{}\".", self.last_query),
                                _ => format!("No logos found for \"{}\". Try the official name.", self.last_query),
                            };
                        }
                        Ok(cands) => {
                            self.status = format!("Found {} candidates for \"{}\".", cands.len(), self.last_query);
                            self.items = cands
                                .into_iter()
                                .map(|cand| Item {
                                    cand,
                                    state: Load::Pending,
                                })
                                .collect();
                            self.fetch_previews();
                        }
                        Err(e) => self.status = format!("Search failed: {e}"),
                    }
                }
                Msg::Loaded(g, i, res) if g == self.generation => {
                    let Some(item) = self.items.get_mut(i) else { continue };
                    item.state = match res {
                        Ok((bytes, raster)) => Load::Ready {
                            transparent: raster.has_transparency(),
                            tex: to_texture(&self.ctx, &format!("logo-{g}-{i}"), &raster),
                            bytes,
                        },
                        Err(e) => Load::Failed(e),
                    };
                }
                Msg::IconSets(Ok(sets)) => self.icon_sets = sets,
                Msg::PhotoResults(g, found, errors) => self.on_photo_results(g, found, errors),
                Msg::PhotoThumb(g, i, res) => self.on_photo_thumb(g, i, res),
                Msg::PhotoCopied(res) => self.on_photo_copied(res),
                Msg::KeyTest(provider, res) => {
                    self.key_tests.insert(provider, Some(res));
                }
                Msg::Status(text) => self.status = text,
                Msg::IconSets(Err(e)) => {
                    self.icon_sets_requested = false; // allow a retry
                    self.status = format!("Could not load icon sets: {e}");
                }
                _ => {} // stale result from an earlier search
            }
        }
    }

    fn request_icon_sets(&mut self) {
        if self.icon_sets_requested {
            return;
        }
        self.icon_sets_requested = true;
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let _ = tx.send(Msg::IconSets(sources::icon_sets(&sources::client())));
            ctx.request_repaint();
        });
    }

    fn set_name(&self, prefix: &str) -> String {
        self.icon_sets
            .iter()
            .find(|s| s.prefix == prefix)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| prefix.to_string())
    }

    fn locked_set_name(&self) -> Option<String> {
        Some(self.set_name(self.settings.icon_set.as_deref()?))
    }

    fn set_icon_set(&mut self, set: Option<String>) {
        if self.settings.icon_set == set {
            return;
        }
        self.settings.icon_set = set;
        if !self.query.trim().is_empty() && !self.searching {
            self.start_search();
        }
        self.status = match self.locked_set_name() {
            Some(name) => format!("Locked to {name}. Every icon search now only shows {name} icons."),
            None => "Showing icons from all sets.".into(),
        };
    }

    fn icon_hex(&self) -> String {
        let [r, g, b] = self.settings.icon_color;
        format!("#{r:02x}{g:02x}{b:02x}")
    }

    /// The bytes to save/copy/convert: icons get the chosen colour baked in.
    fn output_bytes(&self, item: &Item, bytes: &[u8]) -> Vec<u8> {
        if item.cand.tintable() {
            sources::tint(bytes, &self.icon_hex())
        } else {
            bytes.to_vec()
        }
    }

    /// Re-render icon previews after the colour changes (icons are tiny, so this is instant).
    fn retint_previews(&mut self) {
        if self.rendered_color == self.settings.icon_color {
            return;
        }
        self.rendered_color = self.settings.icon_color;
        // A light icon disappears on the light checkerboard, so switch to the dark one.
        let [r, g, b] = self.settings.icon_color.map(f32::from);
        if 0.2126 * r + 0.7152 * g + 0.0722 * b > 180.0 && self.settings.background == Background::LightChecker {
            self.settings.background = Background::DarkChecker;
        }
        let hex = self.icon_hex();
        for (i, item) in self.items.iter_mut().enumerate() {
            if !item.cand.tintable() {
                continue;
            }
            if let Load::Ready { bytes, tex, .. } = &mut item.state
                && let Ok(raster) = convert::preview(&sources::tint(bytes, &hex), true, PREVIEW_PX)
            {
                *tex = to_texture(&self.ctx, &format!("logo-{}-{i}", self.generation), &raster);
            }
        }
    }

    fn file_stem(&self, cand: &Candidate) -> String {
        if cand.tintable() {
            // Icon titles start with the icon name, e.g. "shopping-cart — Material Symbols".
            let name = cand.title.split(" — ").next().unwrap_or("icon");
            return format!("{name}-icon");
        }
        let base: String = self
            .last_query
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let source = cand.source.to_lowercase().replace(' ', "-");
        format!("{}-logo-{source}", base.trim_matches('-'))
    }

    fn save(&mut self, default_name: String, ext: &str, data: Result<Vec<u8>, String>) {
        let data = match data {
            Ok(d) => d,
            Err(e) => {
                self.status = format!("Conversion failed: {e}");
                return;
            }
        };
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{default_name}.{ext}"))
            .add_filter(ext.to_uppercase(), &[ext])
            .save_file()
        else {
            return;
        };
        self.status = match std::fs::write(&path, data) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save {}: {e}", path.display()),
        };
    }

    fn copy_to_clipboard(&mut self, bytes: &[u8], kind: Kind, title: &str) {
        let Some(clipboard) = self.clipboard.as_mut() else {
            self.status = "Clipboard is not available on this system.".into();
            return;
        };
        let (width, height, pixels) = match convert::rgba(bytes, kind == Kind::Svg, self.settings.png_width) {
            Ok(v) => v,
            Err(e) => {
                self.status = format!("Copy failed: {e}");
                return;
            }
        };
        let image = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: pixels.into(),
        };
        self.status = match clipboard.set_image(image) {
            Ok(()) => format!("Copied \"{title}\" to the clipboard ({width}×{height} transparent PNG)."),
            Err(e) => format!("Copy failed: {e}"),
        };
    }

    fn load_convert_file(&mut self, path: PathBuf, bytes: Option<Vec<u8>>) {
        let bytes = match bytes.map(Ok).unwrap_or_else(|| std::fs::read(&path)) {
            Ok(b) => b,
            Err(e) => {
                self.status = format!("Could not read {}: {e}", path.display());
                return;
            }
        };
        match convert::preview(&bytes, true, PREVIEW_PX * 2) {
            Ok(raster) => {
                let tex = to_texture(&self.ctx, "convert-preview", &raster);
                self.status = format!("Loaded {}", path.display());
                self.convert = Some(ConvertFile { path, bytes, tex });
                self.tab = Tab::Convert;
            }
            Err(e) => self.status = format!("{}: {e}", path.display()),
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            let bytes = file.bytes.map(|b| b.to_vec());
            if let Some(path) = file.path {
                self.load_convert_file(path, bytes);
            }
        }
    }

    /// Sidebar for the Icons and Logos tabs: search box, plus colour and icon set for icons.
    fn search_sidebar(&mut self, ui: &mut egui::Ui) {
        theme::sidebar_heading(ui, "Search");
        let hint = match self.settings.mode {
            Mode::Logos => "Company or product name",
            Mode::Icons => "Keyword, e.g. shopping cart",
        };
        let searching = self.searching;
        if theme::sidebar_search(ui, &mut self.query, hint, searching) && !searching {
            self.start_search();
        }
        theme::sidebar_heading(ui, "PNG Export Size");
        self.png_width_field(ui);
        if self.settings.mode == Mode::Icons {
            self.request_icon_sets();
            theme::sidebar_heading(ui, "Colour");
            ui.horizontal(|ui| {
                ui.color_edit_button_srgb(&mut self.settings.icon_color)
                    .on_hover_text("Icon colour, applied to previews, saved files and copies");
                ui.label(egui::RichText::new(self.icon_hex()).monospace().weak());
            });
            theme::sidebar_heading(ui, "Icon Set");
            self.icon_set_picker(ui);
        }
    }

    fn search_tab(&mut self, ui: &mut egui::Ui) {
        self.retint_previews();

        let mut action: Option<SaveAction> = None;
        let mut copy: Option<usize> = None;
        let mut lock: Option<String> = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let gap = 10.0;
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            let cols = (((ui.available_width() + gap) / (self.card_w() + gap)).floor() as usize).max(1);
            for (row, chunk) in self.items.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (j, item) in chunk.iter().enumerate() {
                        self.card(ui, row * cols + j, item, &mut action, &mut copy, &mut lock);
                    }
                });
            }
        });
        if let Some((name, ext, data)) = action {
            self.save(name, ext, data);
        }
        if let Some(i) = copy
            && let Load::Ready { bytes, .. } = &self.items[i].state
        {
            let bytes = self.output_bytes(&self.items[i], bytes);
            let (kind, title) = (self.items[i].cand.kind, self.items[i].cand.title.clone());
            self.copy_to_clipboard(&bytes, kind, &title);
        }
        if lock.is_some() {
            self.set_icon_set(lock);
        }
    }

    fn icon_set_picker(&mut self, ui: &mut egui::Ui) {
        let current = self.locked_set_name().unwrap_or_else(|| "All sets".into());
        let mut chosen: Option<Option<String>> = None;
        egui::ComboBox::from_id_salt("icon-set")
            .selected_text(&current)
            .width(ui.available_width())
            .height(400.0)
            // Default closes on any click, which made the filter box unusable.
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                let filter_box = ui.add(egui::TextEdit::singleline(&mut self.set_filter).hint_text("Filter sets…"));
                if !filter_box.has_focus() && self.set_filter.is_empty() {
                    filter_box.request_focus();
                }
                if ui
                    .selectable_label(self.settings.icon_set.is_none(), "All sets")
                    .clicked()
                {
                    chosen = Some(None);
                    ui.close();
                }
                if self.icon_sets.is_empty() {
                    ui.spinner();
                }
                let filter = self.set_filter.to_lowercase();
                for set in &self.icon_sets {
                    let matches = filter.is_empty()
                        || set.name.to_lowercase().contains(&filter)
                        || set.prefix.contains(&filter)
                        || set.category.to_lowercase().contains(&filter);
                    if !matches {
                        continue;
                    }
                    let selected = self.settings.icon_set.as_deref() == Some(set.prefix.as_str());
                    let clicked = ui
                        .selectable_label(selected, &set.name)
                        .on_hover_text(format!("{} icons · {} · {}", set.total, set.category, set.license))
                        .clicked();
                    if clicked {
                        chosen = Some(Some(set.prefix.clone()));
                        ui.close();
                    }
                }
            });
        if self.settings.icon_set.is_some() {
            ui.label(egui::RichText::new("Results are locked to this set.").small().weak());
            if ui.button("Show All Sets").clicked() {
                chosen = Some(None);
            }
        }
        if let Some(set) = chosen {
            self.set_icon_set(set);
        }
    }

    fn card(
        &self,
        ui: &mut egui::Ui,
        i: usize,
        item: &Item,
        action: &mut Option<SaveAction>,
        copy: &mut Option<usize>,
        lock: &mut Option<String>,
    ) {
        let compact = self.card_w() < COMPACT_W;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(self.card_w() - 18.0);
            ui.vertical(|ui| {
                let ready = matches!(item.state, Load::Ready { .. });
                let (rect, resp) = ui.allocate_exact_size(self.thumb_size(), Sense::click());
                paint_background(ui.painter(), rect, self.settings.background);
                theme::thumb_border(ui, rect);
                match &item.state {
                    Load::Pending => {
                        ui.put(rect, egui::Spinner::new());
                    }
                    Load::Failed(e) => {
                        ui.put(
                            rect,
                            egui::Label::new(egui::RichText::new("Preview unavailable").weak()),
                        )
                        .on_hover_text(e);
                    }
                    Load::Ready { tex, .. } => paint_fit(ui.painter(), rect.shrink(6.0), tex),
                }
                if ready {
                    if resp.hovered() {
                        ui.painter().rect_stroke(
                            rect,
                            6.0,
                            egui::Stroke::new(2.0, theme::accent(ui)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    let resp = resp
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Click to copy as a transparent PNG (right-click for more)");
                    if resp.clicked() {
                        *copy = Some(i);
                    }
                    resp.context_menu(|ui| {
                        if ui.button("Copy as PNG").clicked() {
                            *copy = Some(i);
                            ui.close();
                        }
                        if let Load::Ready { bytes, .. } = &item.state {
                            let ext = item.cand.kind.ext();
                            if ui.button(format!("Save {}…", ext.to_uppercase())).clicked() {
                                *action = Some((self.file_stem(&item.cand), ext, Ok(self.output_bytes(item, bytes))));
                                ui.close();
                            }
                            if item.cand.kind == Kind::Svg
                                && ui
                                    .button(format!("Save as {}px PNG…", self.settings.png_width))
                                    .clicked()
                            {
                                let png = convert::svg_to_png(&self.output_bytes(item, bytes), self.settings.png_width);
                                *action = Some((self.file_stem(&item.cand), "png", png));
                                ui.close();
                            }
                        }
                        ui.hyperlink_to("Open Source in Browser", &item.cand.url);
                        if let Some(set) = &item.cand.set
                            && ui.button(format!("Use {} for all icons", self.set_name(set))).clicked()
                        {
                            *lock = Some(set.clone());
                            ui.close();
                        }
                    });
                }

                ui.add(egui::Label::new(egui::RichText::new(&item.cand.title).strong()).truncate())
                    .on_hover_text(&item.cand.title);
                if compact {
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(item.cand.source).small().weak());
                    ui.label(
                        egui::RichText::new(item.cand.kind.ext().to_uppercase())
                            .small()
                            .monospace(),
                    );
                    if let Load::Ready { transparent, .. } = &item.state {
                        let (txt, col) = if *transparent {
                            ("transparent", theme::green(ui))
                        } else {
                            ("opaque bg", theme::orange(ui))
                        };
                        ui.label(egui::RichText::new(txt).small().color(col));
                    }
                });

                ui.horizontal(|ui| {
                    let bytes = match &item.state {
                        Load::Ready { bytes, .. } => Some(bytes),
                        _ => None,
                    };
                    let ext = item.cand.kind.ext();
                    if ui
                        .add_enabled(
                            bytes.is_some(),
                            egui::Button::new(format!("Save {}", ext.to_uppercase())),
                        )
                        .clicked()
                    {
                        *action = Some((
                            self.file_stem(&item.cand),
                            ext,
                            Ok(self.output_bytes(item, bytes.unwrap())),
                        ));
                    }
                    if item.cand.kind == Kind::Svg
                        && ui
                            .add_enabled(bytes.is_some(), egui::Button::new("→ PNG"))
                            .on_hover_text(format!(
                                "Render to a transparent {}px-wide PNG",
                                self.settings.png_width
                            ))
                            .clicked()
                    {
                        let png =
                            convert::svg_to_png(&self.output_bytes(item, bytes.unwrap()), self.settings.png_width);
                        *action = Some((self.file_stem(&item.cand), "png", png));
                    }
                    ui.hyperlink_to("↗", &item.cand.url)
                        .on_hover_text("Open source in browser");
                });

                if let Some(set) = &item.cand.set {
                    let name = self.set_name(set);
                    if self.settings.icon_set.as_deref() == Some(set.as_str()) {
                        ui.add(
                            egui::Label::new(egui::RichText::new(format!("Locked to {name}")).small().weak())
                                .truncate(),
                        );
                    } else {
                        // Long set names ("Material Symbols Light") would overflow the card.
                        let button = egui::Button::new(format!("Use {name} for All Icons"))
                            .truncate()
                            .min_size(vec2(ui.available_width(), 0.0));
                        if ui
                            .add(button)
                            .on_hover_text(format!(
                                "Only show {name} icons from now on, so your project's icons stay consistent"
                            ))
                            .clicked()
                        {
                            *lock = Some(set.clone());
                        }
                    }
                }
            });
        });
    }

    fn pick_svg(&mut self) {
        if let Some(path) = rfd::FileDialog::new().add_filter("SVG", &["svg"]).pick_file() {
            self.load_convert_file(path, None);
        }
    }

    /// Width used for "→ PNG", copies and the Convert tab.
    fn png_width_field(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut self.settings.png_width)
                    .range(16..=8192)
                    .suffix(" px"),
            )
            .on_hover_text("Width of PNGs rendered from SVGs. Drag or click to type.");
            ui.label(egui::RichText::new("wide").weak());
        });
    }

    /// Sidebar for the Convert tab: pick a file, set the size, save or copy.
    fn convert_sidebar(&mut self, ui: &mut egui::Ui) {
        theme::sidebar_heading(ui, "File");
        let open = egui::Button::new("Open SVG…").min_size(vec2(ui.available_width(), 28.0));
        if ui.add(open).on_hover_text("⌘O").clicked() {
            self.pick_svg();
        }
        match &self.convert {
            Some(file) => {
                let name = file
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate())
                    .on_hover_text(file.path.display().to_string());
                ui.label(egui::RichText::new(human_size(file.bytes.len() as u64)).small().weak());
            }
            None => {
                ui.label(
                    egui::RichText::new("Or drag an .svg file onto the window.")
                        .small()
                        .weak(),
                );
            }
        }

        theme::sidebar_heading(ui, "PNG Export Size");
        self.png_width_field(ui);

        theme::sidebar_heading(ui, "Export");
        let ready = self.convert.is_some();
        let save = theme::primary_button(ui, "Save PNG…").min_size(vec2(ui.available_width(), 28.0));
        if ui.add_enabled(ready, save).clicked() {
            self.save_converted();
        }
        let copy = egui::Button::new("Copy PNG").min_size(vec2(ui.available_width(), 28.0));
        if ui
            .add_enabled(ready, copy)
            .on_hover_text("Copy as a transparent PNG")
            .clicked()
            && let Some(file) = &self.convert
        {
            let (bytes, title) = (file.bytes.clone(), file.path.display().to_string());
            self.copy_to_clipboard(&bytes, Kind::Svg, &title);
        }
    }

    fn save_converted(&mut self) {
        let Some(file) = &self.convert else { return };
        let png = convert::svg_to_png(&file.bytes, self.settings.png_width);
        let stem = file
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "logo".into());
        self.save(stem, "png", png);
    }

    /// Convert tab main area: the preview, or a drop zone when no file is open.
    fn convert_ui(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click());
        match &self.convert {
            Some(file) => {
                paint_background(ui.painter(), rect, self.settings.background);
                theme::thumb_border(ui, rect);
                paint_fit(ui.painter(), rect.shrink(24.0), &file.tex);
            }
            None => {
                let hovering = ui.input(|i| !i.raw.hovered_files.is_empty());
                theme::drop_zone(
                    ui,
                    rect,
                    hovering || resp.hovered(),
                    "Drop an SVG here",
                    "or click to choose a file",
                );
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    self.pick_svg();
                }
            }
        }
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.flush_keys();
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        self.handle_drops(ctx);
        self.grid_zoom(ctx);

        // ⌘, opens Settings, as in every Mac app.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            self.show_settings = true;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::O)) {
            self.pick_svg();
        }

        let bar_fill = theme::bar_fill(ctx);
        let toolbar = egui::Frame::new()
            .fill(bar_fill)
            // Top margin clears the native title bar, which shows the current tab's name.
            .inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: theme::TITLEBAR_H as i8,
                bottom: 4,
            });
        egui::TopBottomPanel::top("toolbar")
            .frame(toolbar)
            .show_separator_line(true)
            .exact_height(theme::TITLEBAR_H + 54.0)
            .show(ctx, |ui| {
                theme::titlebar_drag(ui, ui.max_rect().expand2(vec2(0.0, theme::TITLEBAR_H)));
                theme::window_title(ui, APP_NAME);
                let before = self.tab;
                // Tabs centred on the window, like a Mac settings window.
                let tabs = [
                    (Tab::Icons, "Icons", theme::Glyph::Grid),
                    (Tab::Logos, "Logos", theme::Glyph::Shapes),
                    (Tab::Photos, "Photos", theme::Glyph::Photo),
                    (Tab::Convert, "Convert", theme::Glyph::Convert),
                ];
                let full = ui.max_rect();
                let width = tabs.len() as f32 * 68.0;
                let centre = egui::Rect::from_center_size(full.center(), vec2(width, full.height()));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(centre)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        theme::toolbar_tabs(ui, &mut self.tab, &tabs);
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(full)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        let settings = theme::toolbar_item(ui, theme::Glyph::Gear, "Settings", self.show_settings)
                            .on_hover_text("Settings (⌘,)");
                        if settings.clicked() {
                            if self.show_settings {
                                self.flush_keys();
                            }
                            self.show_settings = !self.show_settings;
                        }
                    },
                );
                if self.tab != before {
                    let mode = match self.tab {
                        Tab::Icons => Some(Mode::Icons),
                        Tab::Logos => Some(Mode::Logos),
                        Tab::Photos | Tab::Convert => None,
                    };
                    if let Some(mode) = mode.filter(|m| *m != self.settings.mode) {
                        self.settings.mode = mode;
                        if !self.query.trim().is_empty() && !self.searching {
                            self.start_search();
                        }
                    }
                }
            });

        let status_bar = egui::Frame::new()
            .fill(bar_fill)
            .inner_margin(egui::Margin::symmetric(12, 4));
        egui::TopBottomPanel::bottom("status")
            .frame(status_bar)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.searching || self.photos.searching {
                        ui.add(egui::Spinner::new().size(12.0));
                    }
                    ui.add(egui::Label::new(egui::RichText::new(&self.status).small().weak()).truncate())
                        .on_hover_text(&self.status);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        theme::size_slider(ui, &mut self.settings.card_w, CARD_W_MIN..=CARD_W_MAX)
                            .on_hover_text("Thumbnail size (⌘+ / ⌘−, or pinch)");
                        ui.add_space(12.0);
                        background_picker(ui, &mut self.settings.background, false);
                        ui.add_space(4.0);
                        let clear = ui
                            .add(egui::Button::new(egui::RichText::new("Clear Cache").small()).frame(false))
                            .on_hover_ui(|ui| {
                                ui.label(format!(
                                    "Searches and downloads are cached on disk for faster repeat lookups ({}).\n\
                                 Clear it to fetch fresh results.",
                                    human_size(cache::size())
                                ));
                            });
                        if clear.clicked() {
                            self.status = match cache::clear() {
                                Ok(()) => "Cache cleared. The next search will fetch fresh results.".into(),
                                Err(e) => format!("Could not clear cache: {e}"),
                            };
                        }
                    });
                });
            });

        let sidebar = egui::Frame::new().fill(theme::sidebar_fill(ctx)).inner_margin(14);
        // egui's own resize handle paints the divider in the text colour (near black) while hovered,
        // so the panel is fixed-width and theme::sidebar_divider draws and drags the edge instead.
        self.settings.sidebar_w = self.settings.sidebar_w.clamp(SIDEBAR_W_MIN, SIDEBAR_W_MAX);
        let panel = egui::SidePanel::left("sidebar")
            .frame(sidebar)
            .resizable(false)
            .show_separator_line(false)
            .exact_width(self.settings.sidebar_w)
            .show(ctx, |ui| match self.tab {
                Tab::Icons | Tab::Logos => self.search_sidebar(ui),
                Tab::Photos => self.photos_sidebar(ui),
                Tab::Convert => self.convert_sidebar(ui),
            });
        if let Some(dx) = theme::sidebar_divider(ctx, panel.response.rect) {
            self.settings.sidebar_w = (self.settings.sidebar_w + dx).clamp(SIDEBAR_W_MIN, SIDEBAR_W_MAX);
        }

        let content = egui::Frame::central_panel(&ctx.style()).inner_margin(16);
        egui::CentralPanel::default()
            .frame(content)
            .show(ctx, |ui| match self.tab {
                Tab::Icons | Tab::Logos => self.search_tab(ui),
                Tab::Photos => self.photos_tab(ui),
                Tab::Convert => self.convert_ui(ui),
            });
        self.settings_window(ctx);
    }
}

fn human_size(bytes: u64) -> String {
    match bytes {
        b if b >= 1 << 20 => format!("{:.1} MB", b as f64 / (1 << 20) as f64),
        b if b >= 1 << 10 => format!("{} KB", b >> 10),
        b => format!("{b} bytes"),
    }
}

fn to_texture(ctx: &egui::Context, name: &str, r: &Raster) -> TextureHandle {
    let img = ColorImage::from_rgba_premultiplied([r.width as usize, r.height as usize], &r.rgba_premul);
    ctx.load_texture(name, img, TextureOptions::LINEAR)
}

/// Control for the preview background (plus a colour swatch when solid).
fn background_picker(ui: &mut egui::Ui, bg: &mut Background, with_label: bool) {
    if let Background::Solid(color) = bg {
        ui.color_edit_button_srgb(color);
    }
    egui::ComboBox::from_id_salt("preview-bg")
        .selected_text(bg.label())
        .show_ui(ui, |ui| {
            ui.selectable_value(bg, Background::LightChecker, Background::LightChecker.label());
            ui.selectable_value(bg, Background::DarkChecker, Background::DarkChecker.label());
            let solid = matches!(bg, Background::Solid(_));
            if ui.selectable_label(solid, "Solid colour").clicked() && !solid {
                *bg = Background::Solid([30, 30, 30]);
            }
        });
    if with_label {
        ui.label("Preview:");
    }
}

fn paint_background(painter: &egui::Painter, rect: Rect, bg: Background) {
    let (a, b) = match bg {
        Background::LightChecker => (Color32::from_gray(255), Color32::from_gray(225)),
        Background::DarkChecker => (Color32::from_gray(40), Color32::from_gray(58)),
        Background::Solid([r, g, b]) => {
            painter.rect_filled(rect, 0.0, Color32::from_rgb(r, g, b));
            return;
        }
    };
    painter.rect_filled(rect, 0.0, a);
    let painter = painter.with_clip_rect(rect);
    let cell = 10.0;
    let (cols, rows) = (
        (rect.width() / cell).ceil() as i32,
        (rect.height() / cell).ceil() as i32,
    );
    for y in 0..rows {
        for x in 0..cols {
            if (x + y) % 2 == 1 {
                let min = rect.min + vec2(x as f32 * cell, y as f32 * cell);
                painter.rect_filled(Rect::from_min_size(min, vec2(cell, cell)), 0.0, b);
            }
        }
    }
}

/// Paint `tex` centred in `rect`, scaled to fit while keeping aspect ratio.
fn paint_fit(painter: &egui::Painter, rect: Rect, tex: &TextureHandle) {
    let [w, h] = tex.size().map(|v| v as f32);
    let scale = (rect.width() / w).min(rect.height() / h);
    let target = Rect::from_center_size(rect.center(), vec2(w * scale, h * scale));
    let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
    painter.image(tex.id(), target, uv, Color32::WHITE);
}
