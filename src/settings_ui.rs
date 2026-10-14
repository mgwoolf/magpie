//! The Settings window: API keys for photo providers plus general preferences.

use std::thread;

use eframe::egui;

use crate::{App, Msg, SettingsTab, background_picker, cache, human_size, photos, secrets, sources, theme};

struct Provider {
    name: &'static str,
    env: &'static str,
    signup: &'static str,
    blurb: &'static str,
}

const PROVIDERS: [Provider; 3] = [
    Provider {
        name: "Pexels",
        env: "PEXELS_API_KEY",
        signup: "https://www.pexels.com/api/new/",
        blurb: "High-quality free photos. Instant key after sign-up.",
    },
    Provider {
        name: "Unsplash",
        env: "UNSPLASH_ACCESS_KEY",
        signup: "https://unsplash.com/oauth/applications/new",
        blurb: "Create an app, then copy its Access Key. Demo apps get 50 searches per hour.",
    },
    Provider {
        name: "Pixabay",
        env: "PIXABAY_API_KEY",
        signup: "https://pixabay.com/api/docs/#api_search_images",
        blurb: "Log in. Your key is shown in the API docs under \"Parameters\".",
    },
];

impl App {
    /// Write edited keys to the keychain.
    pub fn flush_keys(&mut self) {
        if !self.keys_dirty {
            return;
        }
        self.keys_dirty = false;
        let failed = secrets::save(&self.settings.keys);
        if !failed.is_empty() {
            self.status = format!("Could not save API keys to the keychain: {}", failed.join("; "));
        }
    }

    fn key_mut(&mut self, provider: &str) -> &mut String {
        let keys = &mut self.settings.keys;
        match provider {
            "Pexels" => &mut keys.pexels,
            "Unsplash" => &mut keys.unsplash,
            _ => &mut keys.pixabay,
        }
    }

    fn test_key(&mut self, provider: &'static str) {
        let key = match provider {
            "Pexels" => self.settings.keys.resolved().pexels,
            "Unsplash" => self.settings.keys.resolved().unsplash,
            _ => self.settings.keys.resolved().pixabay,
        };
        self.key_tests.insert(provider, None);
        let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
        thread::spawn(move || {
            let res = photos::test_key(&sources::client(), provider, &key);
            let _ = tx.send(Msg::KeyTest(provider, res));
            ctx.request_repaint();
        });
    }

    pub fn settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_settings;
        egui::Window::new(match self.settings_tab {
            SettingsTab::General => "General",
            SettingsTab::ApiKeys => "API Keys",
        })
        .id(egui::Id::new("settings"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            // Fixed width so the window doesn't jump when switching tabs.
            ui.set_width(460.0);
            ui.horizontal(|ui| {
                // Centre the two 64px tabs.
                ui.add_space((ui.available_width() - 132.0) / 2.0);
                theme::toolbar_tabs(
                    ui,
                    &mut self.settings_tab,
                    &[
                        (SettingsTab::General, "General", theme::Glyph::Gear),
                        (SettingsTab::ApiKeys, "API Keys", theme::Glyph::Key),
                    ],
                );
            });
            ui.separator();
            ui.add_space(6.0);
            match self.settings_tab {
                SettingsTab::General => self.general_settings(ui),
                SettingsTab::ApiKeys => self.api_key_settings(ui),
            }
        });
        if self.show_settings && !open {
            self.flush_keys();
        }
        self.show_settings = open;
    }

    fn api_key_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Photo search API keys");
        ui.label(
            egui::RichText::new("Openverse works without a key. Add free keys to also search these services:").weak(),
        );
        ui.add_space(6.0);

        for p in &PROVIDERS {
            theme::card(ui).inner_margin(10).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.strong(p.name);
                    ui.hyperlink_to("Get a free key ↗", p.signup);
                });
                ui.label(egui::RichText::new(p.blurb).small().weak());
                ui.horizontal(|ui| {
                    let reveal = self.reveal_keys;
                    let field = egui::TextEdit::singleline(self.key_mut(p.name))
                        .password(!reveal)
                        .hint_text(format!("Paste your {} key", p.name))
                        .desired_width(260.0);
                    let resp = ui.add(field);
                    if resp.changed() {
                        self.key_tests.remove(p.name);
                        self.keys_dirty = true;
                    }
                    if resp.lost_focus() {
                        self.flush_keys();
                    }
                    if ui.button("Test").clicked() {
                        self.test_key(p.name);
                    }
                    if !self.key_mut(p.name).is_empty() && ui.small_button("✕").on_hover_text("Remove key").clicked()
                    {
                        self.key_mut(p.name).clear();
                        self.key_tests.remove(p.name);
                        self.keys_dirty = true;
                        self.flush_keys();
                    }
                });
                match self.key_tests.get(p.name) {
                    Some(None) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Testing…");
                        });
                    }
                    Some(Some(Ok(n))) => {
                        ui.colored_label(theme::green(ui), format!("✔ Key works ({n} test results)"));
                    }
                    Some(Some(Err(e))) => {
                        ui.colored_label(theme::red(ui), format!("✖ {e}"));
                    }
                    None => {
                        if self.key_mut(p.name).trim().is_empty() && std::env::var(p.env).is_ok_and(|v| !v.is_empty()) {
                            ui.label(egui::RichText::new(format!("Using ${} from the environment", p.env)).small());
                        }
                    }
                }
            });
        }
        ui.checkbox(&mut self.reveal_keys, "Show keys");
        ui.label(
            egui::RichText::new(if cfg!(target_os = "macos") {
                "Keys are stored securely in your macOS Keychain (item \"magpie\"), not in a file."
            } else {
                "Keys are stored securely in your system keychain (service \"magpie\"), not in a file."
            })
            .small()
            .weak(),
        );
    }

    fn general_settings(&mut self, ui: &mut egui::Ui) {
        egui::Grid::new("general")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label("Appearance");
                let before = self.settings.appearance;
                egui::ComboBox::from_id_salt("appearance")
                    .selected_text(before.label())
                    .show_ui(ui, |ui| {
                        for a in theme::Appearance::ALL {
                            ui.selectable_value(&mut self.settings.appearance, a, a.label());
                        }
                    });
                if self.settings.appearance != before {
                    self.settings.appearance.apply(ui.ctx());
                }
                ui.end_row();

                ui.label("PNG export width");
                ui.add(
                    egui::DragValue::new(&mut self.settings.png_width)
                        .range(16..=8192)
                        .suffix(" px"),
                );
                ui.end_row();

                ui.label("Preview background");
                ui.horizontal(|ui| background_picker(ui, &mut self.settings.background, false));
                ui.end_row();

                ui.label("Photos");
                ui.checkbox(&mut self.settings.commercial_only, "Commercial use only");
                ui.end_row();

                ui.label("Cache");
                ui.horizontal(|ui| {
                    ui.label(human_size(cache::size()));
                    if ui.button("Clear cache").clicked() {
                        self.status = match cache::clear() {
                            Ok(()) => "Cache cleared. The next search will fetch fresh results.".into(),
                            Err(e) => format!("Could not clear cache: {e}"),
                        };
                    }
                });
                ui.end_row();
            });
    }
}
