//! The "Photos" tab: stock photo search, preview grid, copy and save.

use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, Rect, Sense, TextureHandle, pos2, vec2};

use crate::photos::{self, Orientation, Photo};
use crate::{App, COMPACT_W, Msg, convert, paint_background, sources, theme, to_texture};

const THUMB_WORKERS: usize = 6;
/// Decode thumbnails a bit larger than the card for sharpness on HiDPI screens.
const THUMB_PX: u32 = 640;

pub struct PhotoItem {
    pub photo: Photo,
    pub thumb: Thumb,
}

pub enum Thumb {
    Pending,
    Failed(String),
    Ready(TextureHandle),
}

#[derive(Default)]
pub struct PhotoState {
    pub query: String,
    pub last_query: String,
    pub page: u32,
    pub generation: u64,
    pub searching: bool,
    /// False once a page comes back empty.
    pub more: bool,
    pub items: Vec<PhotoItem>,
}

enum Action {
    Copy(usize),
    Save(usize),
    Credit(usize),
}

impl App {
    fn photo_query(&self) -> photos::Query {
        photos::Query {
            text: self.photos.last_query.clone(),
            page: self.photos.page,
            orientation: self.settings.photo_orientation,
            commercial_only: self.settings.commercial_only,
        }
    }

    /// Names of the providers that will be searched.
    fn photo_providers(&self) -> Vec<&'static str> {
        let keys = self.settings.keys.resolved();
        let mut names = vec!["Openverse"];
        for (name, key) in [
            ("Pexels", &keys.pexels),
            ("Unsplash", &keys.unsplash),
            ("Pixabay", &keys.pixabay),
        ] {
            if !key.is_empty() {
                names.push(name);
            }
        }
        names
    }

    pub fn start_photo_search(&mut self, more: bool) {
        let st = &mut self.photos;
        if more {
            st.page += 1;
        } else {
            let query = st.query.trim().to_string();
            if query.is_empty() {
                return;
            }
            st.last_query = query;
            st.page = 1;
            st.items.clear();
            st.generation += 1;
        }
        st.searching = true;
        self.status = format!("Searching photos for \"{}\"…", self.photos.last_query);

        let (tx, ctx, generation) = (self.tx.clone(), self.ctx.clone(), self.photos.generation);
        let (query, keys) = (self.photo_query(), self.settings.keys.resolved());
        thread::spawn(move || {
            let (found, errors) = photos::search(&sources::client(), &query, &keys);
            let _ = tx.send(Msg::PhotoResults(generation, found, errors));
            ctx.request_repaint();
        });
    }

    pub fn on_photo_results(&mut self, generation: u64, found: Vec<Photo>, errors: Vec<String>) {
        if generation != self.photos.generation {
            return;
        }
        let st = &mut self.photos;
        st.searching = false;
        st.more = !found.is_empty();
        let start = st.items.len();
        st.items.extend(found.into_iter().map(|photo| PhotoItem {
            photo,
            thumb: Thumb::Pending,
        }));
        let total = st.items.len();

        self.status = if total == 0 && !errors.is_empty() {
            // Nothing came back because the search failed, not because nothing matched.
            format!(
                "Couldn't search photos for \"{}\". {}. Try again in a moment.",
                st.last_query,
                errors.join("; ")
            )
        } else if total == 0 {
            format!("No photos found for \"{}\".", st.last_query)
        } else if errors.is_empty() {
            format!("{total} photos for \"{}\".", st.last_query)
        } else {
            format!("{total} photos for \"{}\".  ⚠ {}", st.last_query, errors.join("; "))
        };
        self.fetch_thumbs(start);
    }

    fn fetch_thumbs(&self, start: usize) {
        let jobs: Vec<(usize, String)> = self.photos.items[start..]
            .iter()
            .enumerate()
            .map(|(i, it)| (start + i, it.photo.thumb_url.clone()))
            .collect();
        let queue = Arc::new(Mutex::new(jobs.into_iter()));
        let client = sources::client();
        for _ in 0..THUMB_WORKERS {
            let (tx, ctx, generation) = (self.tx.clone(), self.ctx.clone(), self.photos.generation);
            let (client, queue) = (client.clone(), queue.clone());
            thread::spawn(move || {
                loop {
                    let Some((i, url)) = queue.lock().unwrap().next() else {
                        break;
                    };
                    let res =
                        sources::download(&client, &url).and_then(|bytes| convert::preview(&bytes, false, THUMB_PX));
                    if tx.send(Msg::PhotoThumb(generation, i, res)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        }
    }

    pub fn on_photo_thumb(&mut self, generation: u64, i: usize, res: Result<convert::Raster, String>) {
        if generation != self.photos.generation {
            return;
        }
        let Some(item) = self.photos.items.get_mut(i) else {
            return;
        };
        item.thumb = match res {
            Ok(r) => Thumb::Ready(to_texture(&self.ctx, &format!("photo-{generation}-{i}"), &r)),
            Err(e) => Thumb::Failed(e),
        };
    }

    fn copy_photo(&mut self, i: usize) {
        let photo = self.photos.items[i].photo.clone();
        let (tx, ctx, keys) = (self.tx.clone(), self.ctx.clone(), self.settings.keys.resolved());
        self.status = format!("Copying photo by {}…", photo.author);
        thread::spawn(move || {
            let client = sources::client();
            let res = sources::download(&client, &photo.large_url).and_then(|bytes| {
                photos::ping_download(&client, &photo, &keys);
                convert::rgba(&bytes, false, 0)
            });
            let _ = tx.send(Msg::PhotoCopied(res.map(|(w, h, px)| (w, h, px, photo.credit))));
            ctx.request_repaint();
        });
    }

    pub fn on_photo_copied(&mut self, res: Result<(u32, u32, Vec<u8>, String), String>) {
        let (width, height, pixels, credit) = match res {
            Ok(v) => v,
            Err(e) => {
                self.status = format!("Copy failed: {e}");
                return;
            }
        };
        let Some(clipboard) = self.clipboard.as_mut() else {
            self.status = "Clipboard is not available on this system.".into();
            return;
        };
        let image = arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: pixels.into(),
        };
        self.status = match clipboard.set_image(image) {
            Ok(()) => format!("Copied {width}×{height} photo. Credit: {credit}"),
            Err(e) => format!("Copy failed: {e}"),
        };
    }

    fn save_photo(&mut self, i: usize) {
        let photo = self.photos.items[i].photo.clone();
        let stem: String = format!("{}-{}-{}", self.photos.last_query, photo.provider, photo.id)
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let ext = photos::extension(&photo.full_url);
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{stem}.{ext}"))
            .add_filter("Image", &[ext])
            .save_file()
        else {
            return;
        };
        self.status = format!("Downloading full-size photo to {}…", path.display());
        let (tx, ctx, keys) = (self.tx.clone(), self.ctx.clone(), self.settings.keys.resolved());
        thread::spawn(move || {
            let client = sources::client();
            let res = sources::download_uncached(&client, &photo.full_url).and_then(|bytes| {
                photos::ping_download(&client, &photo, &keys);
                std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
                Ok(format!("Saved {}. Credit: {}", path.display(), photo.credit))
            });
            let _ = tx.send(Msg::Status(res.unwrap_or_else(|e| format!("Save failed: {e}"))));
            ctx.request_repaint();
        });
    }

    fn copy_credit(&mut self, i: usize) {
        let credit = self.photos.items[i].photo.credit.clone();
        self.status = match self.clipboard.as_mut().map(|c| c.set_text(credit.clone())) {
            Some(Ok(())) => format!("Copied credit: {credit}"),
            Some(Err(e)) => format!("Copy failed: {e}"),
            None => "Clipboard is not available on this system.".into(),
        };
    }

    /// Sidebar for the Photos tab: search box, filters and the providers being searched.
    pub fn photos_sidebar(&mut self, ui: &mut egui::Ui) {
        theme::sidebar_heading(ui, "Search");
        let searching = self.photos.searching;
        if theme::sidebar_search(ui, &mut self.photos.query, "Describe the photo", searching) && !searching {
            self.start_photo_search(false);
        }

        let mut rerun = false;
        theme::sidebar_heading(ui, "Orientation");
        egui::ComboBox::from_id_salt("orientation")
            .width(ui.available_width())
            .selected_text(self.settings.photo_orientation.label())
            .show_ui(ui, |ui| {
                for o in Orientation::ALL {
                    rerun |= ui
                        .selectable_value(&mut self.settings.photo_orientation, o, o.label())
                        .changed();
                }
            });
        theme::sidebar_heading(ui, "License");
        rerun |= ui
            .checkbox(&mut self.settings.commercial_only, "Commercial use only")
            .on_hover_text(
                "Hide Openverse photos whose license forbids commercial use.\n\
                 Pexels, Unsplash and Pixabay photos are always free for commercial use.",
            )
            .changed();
        if rerun && !self.photos.last_query.is_empty() && !self.photos.searching {
            self.photos.query = self.photos.last_query.clone();
            self.start_photo_search(false);
        }

        theme::sidebar_heading(ui, "Sources");
        for name in self.photo_providers() {
            ui.label(name);
        }
        if self.photo_providers().len() < 4 && ui.link("Add Pexels / Unsplash / Pixabay keys…").clicked() {
            self.show_settings = true;
            self.settings_tab = crate::SettingsTab::ApiKeys;
        }
    }

    pub fn photos_tab(&mut self, ui: &mut egui::Ui) {
        let mut action = None;
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let gap = 10.0;
            ui.spacing_mut().item_spacing = vec2(gap, gap);
            let cols = (((ui.available_width() + gap) / (self.card_w() + gap)).floor() as usize).max(1);
            for (row, chunk) in self.photos.items.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    for (j, item) in chunk.iter().enumerate() {
                        if let Some(a) = self.photo_card(ui, row * cols + j, item) {
                            action = Some(a);
                        }
                    }
                });
            }
            if !self.photos.items.is_empty() {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if self.photos.searching {
                        ui.spinner();
                    } else if self.photos.more && ui.button("Load more").clicked() {
                        self.start_photo_search(true);
                    } else if !self.photos.more {
                        ui.label(egui::RichText::new("No more results.").weak());
                    }
                });
            }
        });
        match action {
            Some(Action::Copy(i)) => self.copy_photo(i),
            Some(Action::Save(i)) => self.save_photo(i),
            Some(Action::Credit(i)) => self.copy_credit(i),
            None => {}
        }
    }

    fn photo_card(&self, ui: &mut egui::Ui, i: usize, item: &PhotoItem) -> Option<Action> {
        let mut action = None;
        let p = &item.photo;
        let compact = self.card_w() < COMPACT_W;
        theme::card(ui).show(ui, |ui| {
            ui.set_width(self.card_w() - 18.0);
            ui.vertical(|ui| {
                let (rect, resp) = ui.allocate_exact_size(self.thumb_size(), Sense::click());
                paint_background(ui.painter(), rect, self.settings.background);
                match &item.thumb {
                    Thumb::Pending => {
                        ui.put(rect, egui::Spinner::new());
                    }
                    Thumb::Failed(e) => {
                        ui.put(
                            rect,
                            egui::Label::new(egui::RichText::new("Preview unavailable").weak()),
                        )
                        .on_hover_text(e);
                    }
                    Thumb::Ready(tex) => {
                        paint_cover(ui.painter(), rect, tex);
                        theme::thumb_border(ui, rect);
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
                            .on_hover_text("Click to copy the photo (right-click for more)");
                        if resp.clicked() {
                            action = Some(Action::Copy(i));
                        }
                        resp.context_menu(|ui| {
                            if ui.button("Copy photo").clicked() {
                                action = Some(Action::Copy(i));
                                ui.close();
                            }
                            if ui.button("Copy credit line").clicked() {
                                action = Some(Action::Credit(i));
                                ui.close();
                            }
                            if ui.button("Save full size…").clicked() {
                                action = Some(Action::Save(i));
                                ui.close();
                            }
                        });
                    }
                }

                let title = if p.title.is_empty() { "Untitled" } else { &p.title };
                ui.add(egui::Label::new(egui::RichText::new(title).strong()).truncate())
                    .on_hover_text(title);
                if compact {
                    return;
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{} · {}", p.author, p.provider))
                            .small()
                            .weak(),
                    )
                    .truncate(),
                );
                ui.horizontal(|ui| {
                    let ok = !p.license.contains("NC") && !p.license.contains("ND");
                    let color = if ok { theme::green(ui) } else { theme::orange(ui) };
                    ui.label(egui::RichText::new(&p.license).small().color(color))
                        .on_hover_text(if ok {
                            "Free to use commercially (check attribution requirements)"
                        } else {
                            "NC = non-commercial only, ND = no modifications"
                        });
                    if p.width > 0 {
                        ui.label(egui::RichText::new(format!("{}×{}", p.width, p.height)).small().weak());
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button("Save")
                        .on_hover_text("Download the full-size photo")
                        .clicked()
                    {
                        action = Some(Action::Save(i));
                    }
                    if ui
                        .button("Credit")
                        .on_hover_text(format!("Copy: {}", p.credit))
                        .clicked()
                    {
                        action = Some(Action::Credit(i));
                    }
                    ui.hyperlink_to("↗", &p.page_url).on_hover_text("Open photo page");
                });
            });
        });
        action
    }
}

/// Paint `tex` filling `rect`, cropping the overflow (like CSS `object-fit: cover`).
fn paint_cover(painter: &egui::Painter, rect: Rect, tex: &TextureHandle) {
    let [w, h] = tex.size().map(|v| v as f32);
    let (img_aspect, rect_aspect) = (w / h, rect.width() / rect.height());
    let uv = if img_aspect > rect_aspect {
        let visible = rect_aspect / img_aspect;
        Rect::from_min_max(pos2((1.0 - visible) / 2.0, 0.0), pos2((1.0 + visible) / 2.0, 1.0))
    } else {
        let visible = img_aspect / rect_aspect;
        Rect::from_min_max(pos2(0.0, (1.0 - visible) / 2.0), pos2(1.0, (1.0 + visible) / 2.0))
    };
    painter.image(tex.id(), rect, uv, Color32::WHITE);
}
