//! macOS look and feel: the system font, AppKit-like colours for light and dark mode,
//! and a few widgets egui lacks (segmented control, accent button, cards).

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Response, Sense, Shadow,
    Stroke, TextStyle, Theme, Ui, Visuals, vec2,
};

/// Height of the title bar the window content is drawn behind, where the traffic lights sit.
pub const TITLEBAR_H: f32 = 28.0;

struct Palette {
    window: Color32,
    content: Color32,
    card: Color32,
    control: Color32,
    control_hover: Color32,
    control_active: Color32,
    control_stroke: Color32,
    separator: Color32,
    text: Color32,
    secondary: Color32,
    accent: Color32,
    green: Color32,
    orange: Color32,
    red: Color32,
}

const LIGHT: Palette = Palette {
    window: Color32::from_gray(236),
    content: Color32::from_gray(255),
    card: Color32::from_gray(245),
    control: Color32::from_gray(255),
    control_hover: Color32::from_gray(246),
    control_active: Color32::from_gray(225),
    control_stroke: Color32::from_rgba_premultiplied(0, 0, 0, 38),
    separator: Color32::from_rgba_premultiplied(0, 0, 0, 26),
    text: Color32::from_gray(30),
    secondary: Color32::from_gray(128),
    accent: Color32::from_rgb(0, 122, 255),
    green: Color32::from_rgb(36, 138, 61),
    orange: Color32::from_rgb(199, 110, 0),
    red: Color32::from_rgb(215, 0, 21),
};

const DARK: Palette = Palette {
    window: Color32::from_gray(46),
    content: Color32::from_gray(30),
    card: Color32::from_gray(42),
    control: Color32::from_gray(82),
    control_hover: Color32::from_gray(92),
    control_active: Color32::from_gray(110),
    control_stroke: Color32::from_rgba_premultiplied(0, 0, 0, 0),
    separator: Color32::from_rgba_premultiplied(18, 18, 18, 18),
    text: Color32::from_gray(232),
    secondary: Color32::from_gray(152),
    accent: Color32::from_rgb(10, 132, 255),
    green: Color32::from_rgb(48, 209, 88),
    orange: Color32::from_rgb(255, 159, 10),
    red: Color32::from_rgb(255, 69, 58),
};

fn palette(ui: &Ui) -> &'static Palette {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

pub fn accent(ui: &Ui) -> Color32 {
    palette(ui).accent
}
pub fn green(ui: &Ui) -> Color32 {
    palette(ui).green
}
pub fn orange(ui: &Ui) -> Color32 {
    palette(ui).orange
}
pub fn red(ui: &Ui) -> Color32 {
    palette(ui).red
}
/// Background for the toolbar and status bar, which frame the white content area.
pub fn bar_fill(ctx: &egui::Context) -> Color32 {
    if ctx.style().visuals.dark_mode {
        DARK.window
    } else {
        LIGHT.window
    }
}

/// Light, dark, or follow macOS ("Auto").
#[derive(Clone, Copy, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Appearance {
    #[default]
    Auto,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::Auto, Appearance::Light, Appearance::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Appearance::Auto => "Auto",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    /// Switch egui's colours and the native title bar to match.
    pub fn apply(self, ctx: &egui::Context) {
        let (theme, system) = match self {
            Appearance::Auto => (egui::ThemePreference::System, egui::SystemTheme::SystemDefault),
            Appearance::Light => (egui::ThemePreference::Light, egui::SystemTheme::Light),
            Appearance::Dark => (egui::ThemePreference::Dark, egui::SystemTheme::Dark),
        };
        ctx.set_theme(theme);
        ctx.send_viewport_cmd(egui::ViewportCommand::SetTheme(system));
    }
}

/// Use San Francisco and AppKit styling, following the system light/dark setting.
pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    for (theme, p) in [(Theme::Light, &LIGHT), (Theme::Dark, &DARK)] {
        ctx.set_style_of(
            theme,
            style(ctx.style_of(theme).as_ref().clone(), p, theme == Theme::Dark),
        );
    }
    ctx.set_theme(egui::ThemePreference::System);
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    // egui's bundled fonts stay as fallbacks for glyphs SF lacks (symbols, emoji).
    for (name, path, family) in [
        ("sf-pro", "/System/Library/Fonts/SFNS.ttf", FontFamily::Proportional),
        ("sf-mono", "/System/Library/Fonts/SFNSMono.ttf", FontFamily::Monospace),
    ] {
        let Ok(bytes) = std::fs::read(path) else { continue };
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_owned(bytes)));
        fonts.families.entry(family).or_default().insert(0, name.into());
    }
    ctx.set_fonts(fonts);
}

fn style(mut style: egui::Style, p: &Palette, dark: bool) -> egui::Style {
    use FontFamily::{Monospace, Proportional};
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, Proportional)),
        (TextStyle::Body, FontId::new(13.0, Proportional)),
        (TextStyle::Button, FontId::new(13.0, Proportional)),
        (TextStyle::Heading, FontId::new(15.0, Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, Monospace)),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = vec2(8.0, 6.0);
    s.button_padding = vec2(10.0, 3.0);
    s.interact_size.y = 22.0;
    s.window_margin = Margin::same(16);
    s.menu_margin = Margin::same(5);
    s.combo_height = 300.0;
    s.scroll = egui::style::ScrollStyle::floating();
    style.animation_time = 0.12;

    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.panel_fill = p.content;
    v.window_fill = p.window;
    v.faint_bg_color = p.card;
    v.extreme_bg_color = p.content;
    v.text_edit_bg_color = Some(if dark { Color32::from_gray(38) } else { Color32::WHITE });
    v.code_bg_color = p.card;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.orange;
    v.error_fg_color = p.red;
    v.weak_text_color = Some(p.secondary);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(6);
    v.window_stroke = Stroke::new(1.0, p.separator);
    v.window_shadow = Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 0,
        color: Color32::from_black_alpha(60),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(40),
    };
    v.window_highlight_topmost = false;
    v.collapsing_header_frame = false;
    v.indent_has_left_vline = false;
    // Selected rows and the text-selection highlight: a tint of the accent, with accent text.
    v.selection.bg_fill = p.accent.gamma_multiply(if dark { 0.35 } else { 0.18 });
    v.selection.stroke = Stroke::new(
        1.5,
        if dark {
            Color32::from_rgb(120, 180, 255)
        } else {
            p.accent
        },
    );
    v.text_cursor.stroke = Stroke::new(1.5, p.accent);

    let radius = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.window;
    w.noninteractive.weak_bg_fill = p.window;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = radius;
    for (wv, fill) in [
        (&mut w.inactive, p.control),
        (&mut w.hovered, p.control_hover),
        (&mut w.active, p.control_active),
        (&mut w.open, p.control_hover),
    ] {
        wv.bg_fill = fill;
        wv.weak_bg_fill = fill;
        wv.bg_stroke = Stroke::new(1.0, p.control_stroke);
        wv.fg_stroke = Stroke::new(1.0, p.text);
        wv.corner_radius = radius;
        // AppKit controls don't grow on hover.
        wv.expansion = 0.0;
    }
    style.visuals = v;
    style
}

/// Sidebar background: a shade between the toolbar and the white content area.
pub fn sidebar_fill(ctx: &egui::Context) -> Color32 {
    if ctx.style().visuals.dark_mode {
        Color32::from_gray(38)
    } else {
        Color32::from_gray(242)
    }
}

/// Hairline between the sidebar and the content, with a drag handle to resize the sidebar.
/// Call between showing the sidebar and the central panel. Returns how far it was dragged
/// this frame. Only the cursor changes on hover, as in Finder.
pub fn sidebar_divider(ctx: &egui::Context, sidebar: egui::Rect) -> Option<f32> {
    let x = sidebar.right();
    // Same layer as the panels, so menus and the Settings window stay above the handle.
    let grab = egui::Rect::from_x_y_ranges(x - 4.0..=x + 4.0, sidebar.y_range());
    let builder = egui::UiBuilder::new()
        .layer_id(egui::LayerId::background())
        .max_rect(grab);
    let mut ui = Ui::new(ctx.clone(), egui::Id::new("sidebar-divider"), builder);
    let resp = ui.allocate_rect(grab, Sense::drag());

    let dark = ctx.style().visuals.dark_mode;
    let line = if dark {
        Color32::from_gray(20)
    } else {
        Color32::from_gray(214)
    };
    ui.painter()
        .with_clip_rect(grab)
        .vline(x - 0.5, sidebar.y_range(), Stroke::new(1.0, line));

    if resp.hovered() || resp.dragged() {
        ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    resp.dragged().then(|| resp.drag_delta().x)
}

/// Small, bold, grey section title, as in Finder and Mail sidebars.
pub fn sidebar_heading(ui: &mut Ui, text: &str) {
    ui.add_space(if ui.min_rect().height() > 1.0 { 14.0 } else { 0.0 });
    let color = palette(ui).secondary;
    ui.label(egui::RichText::new(text).size(11.0).strong().color(color));
    ui.add_space(2.0);
}

/// Full-width search field with a Search button beneath. True when the user submits.
pub fn sidebar_search(ui: &mut Ui, query: &mut String, hint: &str, busy: bool) -> bool {
    let enter = search_field(ui, query, hint);
    ui.add_space(4.0);
    let clicked = search_button(ui, busy).clicked();
    enter || clicked
}

/// Full-width accent button with a magnifying glass before "Search".
fn search_button(ui: &mut Ui, busy: bool) -> Response {
    let p = palette(ui);
    let sense = if busy { Sense::hover() } else { Sense::click() };
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), sense);
    let fill = if busy {
        p.accent.gamma_multiply(0.5)
    } else if resp.is_pointer_button_down_on() {
        p.accent.gamma_multiply(0.8)
    } else {
        p.accent
    };
    ui.painter().rect_filled(rect, 6.0, fill);

    let label = if busy { "Searching…" } else { "Search" };
    let galley = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), Color32::WHITE);
    let (icon_w, gap) = (13.0, 6.0);
    let left = rect.center().x - (icon_w + gap + galley.size().x) / 2.0;
    let c = egui::pos2(left + 5.0, rect.center().y - 1.0);
    let glass = Stroke::new(1.6, Color32::WHITE);
    ui.painter().circle_stroke(c, 4.5, glass);
    ui.painter()
        .line_segment([c + vec2(3.3, 3.3), c + vec2(6.5, 6.5)], glass);
    let text_pos = egui::pos2(left + icon_w + gap, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley(text_pos, galley, Color32::WHITE);
    resp
}

/// A large, high-contrast search field: magnifying glass, clear button and an accent focus ring.
/// Returns true when Return is pressed in it.
pub fn search_field(ui: &mut Ui, query: &mut String, hint: &str) -> bool {
    let p = palette(ui);
    let dark = ui.visuals().dark_mode;
    let id = ui.id().with("search-field");
    let focused = ui.memory(|m| m.has_focus(id));
    let fill = if dark { Color32::from_gray(24) } else { Color32::WHITE };
    let stroke = if focused {
        Stroke::new(2.0, p.accent)
    } else {
        Stroke::new(
            1.0,
            if dark {
                Color32::from_gray(90)
            } else {
                Color32::from_gray(185)
            },
        )
    };
    let mut enter = false;
    let frame = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(8)
        .inner_margin(Margin::symmetric(8, 7));
    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            // Magnifying glass.
            let (icon, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
            let glass = Stroke::new(1.6, if focused { p.accent } else { p.secondary });
            let c = icon.center() - vec2(1.5, 1.5);
            ui.painter().circle_stroke(c, 5.0, glass);
            ui.painter()
                .line_segment([c + vec2(3.6, 3.6), c + vec2(7.0, 7.0)], glass);

            let clear_w = if query.is_empty() { 0.0 } else { 20.0 };
            let resp = ui.add(
                egui::TextEdit::singleline(query)
                    .id(id)
                    .frame(false)
                    .hint_text(egui::RichText::new(hint).color(p.secondary))
                    .font(FontId::proportional(14.0))
                    .margin(Margin::ZERO)
                    .desired_width(ui.available_width() - clear_w),
            );
            enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if !query.is_empty() {
                let (rect, clear) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                let clear = clear
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Clear");
                ui.painter()
                    .circle_filled(rect.center(), 7.0, p.secondary.gamma_multiply(0.7));
                let x = Stroke::new(1.4, fill);
                let d = 2.5;
                ui.painter()
                    .line_segment([rect.center() - vec2(d, d), rect.center() + vec2(d, d)], x);
                ui.painter()
                    .line_segment([rect.center() + vec2(-d, d), rect.center() + vec2(d, -d)], x);
                if clear.clicked() {
                    query.clear();
                    resp.request_focus();
                }
            }
        });
    });
    enter
}

/// Dashed, rounded drop target with a title and hint, highlighted in the accent colour when active.
pub fn drop_zone(ui: &Ui, rect: egui::Rect, active: bool, title: &str, hint: &str) {
    let p = palette(ui);
    let color = if active {
        p.accent
    } else {
        p.secondary.gamma_multiply(0.6)
    };
    let zone = rect.shrink(8.0);
    if active {
        ui.painter().rect_filled(zone, 12.0, p.accent.gamma_multiply(0.06));
    }
    // Dashed outline: straight edges only, corners left open like Finder's drop targets.
    let stroke = Stroke::new(1.5, color);
    let (r, inset) = (zone, 12.0);
    for (a, b) in [
        (r.left_top() + vec2(inset, 0.0), r.right_top() - vec2(inset, 0.0)),
        (r.left_bottom() + vec2(inset, 0.0), r.right_bottom() - vec2(inset, 0.0)),
        (r.left_top() + vec2(0.0, inset), r.left_bottom() - vec2(0.0, inset)),
        (r.right_top() + vec2(0.0, inset), r.right_bottom() - vec2(0.0, inset)),
    ] {
        ui.painter().extend(egui::Shape::dashed_line(&[a, b], stroke, 6.0, 4.0));
    }
    let c = zone.center();
    paint_glyph(
        ui.painter(),
        egui::Rect::from_center_size(c - vec2(0.0, 30.0), vec2(28.0, 28.0)),
        Glyph::Convert,
        color,
    );
    ui.painter().text(
        c + vec2(0.0, 2.0),
        egui::Align2::CENTER_CENTER,
        title,
        FontId::proportional(15.0),
        p.text,
    );
    ui.painter().text(
        c + vec2(0.0, 22.0),
        egui::Align2::CENTER_CENTER,
        hint,
        FontId::proportional(12.0),
        p.secondary,
    );
}

/// The rounded tile used for result cards and grouped settings.
pub fn card(ui: &Ui) -> egui::Frame {
    let p = palette(ui);
    egui::Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.separator))
        .corner_radius(10)
        .inner_margin(8)
}

/// A filled, accent-coloured default button (the one Return triggers).
pub fn primary_button(ui: &Ui, text: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(text).color(Color32::WHITE))
        .fill(palette(ui).accent)
        .stroke(Stroke::NONE)
}

/// Title centred on the whole window in the title bar strip, like a native window title.
/// Dimmed while the app is in the background, as AppKit does.
pub fn window_title(ui: &Ui, title: &str) {
    let p = palette(ui);
    let focused = ui.input(|i| i.viewport().focused.unwrap_or(true));
    let centre = egui::pos2(ui.ctx().content_rect().center().x, TITLEBAR_H / 2.0);
    ui.painter().text(
        centre,
        egui::Align2::CENTER_CENTER,
        title,
        FontId::proportional(13.0),
        if focused { p.text } else { p.secondary },
    );
}

/// Make the toolbar behave like a title bar: drag to move the window, double-click to zoom.
/// Call before adding the toolbar's widgets so they take precedence.
pub fn titlebar_drag(ui: &Ui, rect: egui::Rect) -> Response {
    let resp = ui.interact(rect, ui.id().with("titlebar"), Sense::click_and_drag());
    if resp.drag_started_by(egui::PointerButton::Primary) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if resp.double_clicked() {
        let zoomed = ui.input(|i| i.viewport().maximized.unwrap_or(false));
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!zoomed));
    }
    resp
}

/// Hairline edge so light previews don't bleed into the card.
pub fn thumb_border(ui: &Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0, palette(ui).separator),
        egui::StrokeKind::Inside,
    );
}

/// Line-art toolbar icons in the style of SF Symbols.
#[derive(Clone, Copy)]
pub enum Glyph {
    /// square.grid.2x2
    Grid,
    /// square.on.circle
    Shapes,
    /// photo
    Photo,
    /// gearshape
    Gear,
    /// key
    Key,
    /// A page with an arrow, for SVG → PNG.
    Convert,
}

fn paint_glyph(painter: &egui::Painter, rect: egui::Rect, glyph: Glyph, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let c = rect.center();
    match glyph {
        Glyph::Grid => {
            let (cell, gap) = (8.0, 3.0);
            for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let center = c + vec2(dx, dy) * (cell + gap) / 2.0;
                let r = egui::Rect::from_center_size(center, vec2(cell, cell));
                painter.rect_stroke(r, 2.0, stroke, egui::StrokeKind::Middle);
            }
        }
        Glyph::Shapes => {
            painter.circle_stroke(c + vec2(3.5, 3.5), 6.5, stroke);
            let square = egui::Rect::from_center_size(c - vec2(3.0, 3.0), vec2(12.0, 12.0));
            painter.rect_stroke(square, 2.5, stroke, egui::StrokeKind::Middle);
        }
        Glyph::Gear => {
            for k in 0..8 {
                let dir = egui::Vec2::angled(k as f32 * std::f32::consts::TAU / 8.0);
                painter.line_segment([c + dir * 7.0, c + dir * 10.0], Stroke::new(3.0, color));
            }
            painter.circle_stroke(c, 7.0, stroke);
            painter.circle_stroke(c, 3.0, stroke);
        }
        Glyph::Key => {
            let head = c + vec2(-5.0, -4.0);
            painter.circle_stroke(head, 5.0, stroke);
            painter.circle_filled(head + vec2(-1.2, -1.2), 1.3, color);
            let tip = c + vec2(8.0, 9.0);
            painter.line_segment([head + vec2(3.5, 3.5), tip], stroke);
            painter.line_segment([tip - vec2(3.0, 3.0), tip - vec2(3.0, 3.0) + vec2(2.8, -2.8)], stroke);
            painter.line_segment([tip - vec2(0.5, 0.5), tip - vec2(0.5, 0.5) + vec2(2.8, -2.8)], stroke);
        }
        Glyph::Convert => {
            let page = egui::Rect::from_center_size(c - vec2(3.0, 0.0), vec2(13.0, 17.0));
            painter.rect_stroke(page, 2.5, stroke, egui::StrokeKind::Middle);
            let y = c.y + 1.0;
            let tip = egui::pos2(c.x + 10.0, y);
            painter.line_segment([egui::pos2(c.x - 1.0, y), tip], stroke);
            painter.line_segment([tip, tip + vec2(-3.5, -3.5)], stroke);
            painter.line_segment([tip, tip + vec2(-3.5, 3.5)], stroke);
        }
        Glyph::Photo => {
            let frame = egui::Rect::from_center_size(c, vec2(20.0, 16.0));
            painter.rect_stroke(frame, 3.0, stroke, egui::StrokeKind::Middle);
            painter.circle_filled(frame.min + vec2(14.5, 5.0), 1.8, color);
            let base = frame.bottom() - 3.0;
            painter.line(
                vec![
                    egui::pos2(frame.left() + 3.0, base),
                    egui::pos2(frame.left() + 8.0, frame.top() + 6.5),
                    egui::pos2(frame.left() + 12.0, base - 3.5),
                    egui::pos2(frame.left() + 14.0, base - 5.0),
                    egui::pos2(frame.right() - 3.0, base),
                ],
                stroke,
            );
        }
    }
}

/// A toolbar item: icon over label, on a rounded highlight when `selected`.
pub fn toolbar_item(ui: &mut Ui, glyph: Glyph, label: &str, selected: bool) -> Response {
    let p = palette(ui);
    let dark = ui.visuals().dark_mode;
    let (rect, resp) = ui.allocate_exact_size(vec2(64.0, 46.0), Sense::click());
    let highlight = if dark {
        Color32::from_white_alpha(22)
    } else {
        Color32::from_black_alpha(16)
    };
    if selected {
        ui.painter().rect_filled(rect, 6.0, highlight);
    } else if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 6.0, highlight.gamma_multiply(0.6));
    }
    let icon_rect = egui::Rect::from_min_size(rect.min + vec2(0.0, 5.0), vec2(rect.width(), 22.0));
    paint_glyph(
        ui.painter(),
        icon_rect,
        glyph,
        if selected { p.accent } else { p.secondary },
    );
    ui.painter().text(
        egui::pos2(rect.center().x, rect.bottom() - 6.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        FontId::proportional(11.0),
        if selected { p.text } else { p.secondary },
    );
    resp
}

/// Preferences-style toolbar tabs built from [`toolbar_item`]. Returns true when the selection changed.
pub fn toolbar_tabs<T: PartialEq + Copy>(ui: &mut Ui, value: &mut T, items: &[(T, &str, Glyph)]) -> bool {
    ui.spacing_mut().item_spacing.x = 4.0;
    let mut changed = false;
    for (option, label, glyph) in items {
        let selected = *value == *option;
        if toolbar_item(ui, *glyph, label, selected).clicked() && !selected {
            *value = *option;
            changed = true;
        }
    }
    changed
}
/// Finder-style size slider: a small square, the slider, a large square.
pub fn size_slider(ui: &mut Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) -> Response {
    let color = palette(ui).secondary;
    let square = |ui: &mut Ui, side: f32| {
        let (rect, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
        let r = egui::Rect::from_center_size(rect.center(), vec2(side, side));
        ui.painter()
            .rect_stroke(r, 1.5, Stroke::new(1.2, color), egui::StrokeKind::Middle);
    };
    // Laid out right to left (status bar), so the large square comes first.
    square(ui, 11.0);
    ui.spacing_mut().slider_width = 90.0;
    let resp = ui.add(egui::Slider::new(value, range).show_value(false));
    square(ui, 6.0);
    resp
}
