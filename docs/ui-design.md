# UI design system

Magpie should feel like a native Mac app, even though it's drawn by egui. Everything that
defines the look lives in `src/theme.rs`. **Reuse it; don't restyle widgets ad hoc.**

## Principles

- AppKit conventions: a unified toolbar with icon-over-label tabs, a grey sidebar with small
  section headings, white content, and a slim status bar. Controls don't grow on hover.
- One accent colour (system blue) for primary actions, focus and selection.
- Everything works in **light and dark**. `Appearance` (Auto/Light/Dark) is a user setting.
- No emoji in the UI. Icons are line-art `Glyph`s drawn with the painter, in an SF Symbols style.
- Text: SF Pro 13 pt body, 11 pt small and labels, 15 pt headings, SF Mono for monospace.
  Loaded from `/System/Library/Fonts`; egui's bundled fonts are the fallback (non-macOS too).

## Colours

`Palette` has a `LIGHT` and a `DARK` constant. Read colours through helpers, never hard-code them:

| Helper | Use |
|--------|-----|
| `theme::accent(ui)` | primary buttons, focus rings, selected icons, hover outlines |
| `theme::green/orange/red(ui)` | success, warning (e.g. "opaque bg", non-commercial license), error |
| `theme::bar_fill(ctx)` | toolbar and status bar background |
| `theme::sidebar_fill(ctx)` | sidebar background |
| `ui.visuals().weak_text_color()` / `RichText::weak()` | secondary text |

Widget visuals (`style()` in theme.rs) set 6 px radii, `expansion = 0`, and a translucent accent
for `selection.bg_fill` with an accent `selection.stroke`. That stroke doubles as the text-edit
focus ring and the selected-label text colour, so a white stroke would break both.

## Widgets

| Function | What it draws |
|----------|---------------|
| `toolbar_item(ui, glyph, label, selected)` | 64×46 toolbar item: glyph over an 11 pt label, rounded highlight when selected |
| `toolbar_tabs(ui, &mut value, &[(T, label, Glyph)])` | a row of `toolbar_item`s acting as tabs |
| `window_title(ui, title)` | the title centred on the whole window in the title-bar strip; dims when unfocused |
| `titlebar_drag(ui, rect)` | drag-to-move and double-click-to-zoom. **Call before** adding the panel's widgets |
| `sidebar_heading(ui, text)` | small bold grey section title with spacing above (none for the first) |
| `sidebar_search(ui, &mut q, hint, busy)` | `search_field` plus the full-width `search_button`. Returns true on submit |
| `search_field(ui, &mut q, hint)` | large white field with magnifier, clear (✕) button and 2 px accent focus ring |
| `primary_button(ui, text)` | accent-filled `egui::Button` with white text |
| `card(ui)` | rounded tile frame for result cards and grouped settings |
| `thumb_border(ui, rect)` | hairline around a preview so light images don't bleed into the card |
| `size_slider(ui, &mut v, range)` | Finder-style slider between a small and a large square (right-to-left layouts) |
| `drop_zone(ui, rect, active, title, hint)` | dashed drop target with the Convert glyph |
| `sidebar_divider(ctx, rect)` | grey hairline plus drag handle on the sidebar's edge. Returns drag delta |
| `Glyph::{Grid, Shapes, Photo, Convert, Gear, Key}` | line-art icons drawn by `paint_glyph` (about 22 px box) |

### Adding a glyph

Add a variant to `Glyph` and a match arm in `paint_glyph`. Draw with `Stroke::new(1.6, color)`
inside roughly a 20×20 area around `rect.center()`, using rounded rects, circles and line
segments. Keep it simple and outline-only to match the existing glyphs.

## Layout conventions

- **Toolbar:** tabs centred on the window; Settings at the far right. The left side is empty.
- **Sidebar:** 14 px padding; sections are `sidebar_heading` + controls; full-width inputs; the
  primary action goes in the sidebar (Search, Save PNG…).
- **Content:** 16 px padding. Result grids are rows of fixed-width cards. Card width comes from
  `Settings.card_w` (slider, ⌘+/−/0, pinch). Below `COMPACT_W` only the thumbnail and title
  show, and every action must also be in the right-click menu.
- **Status bar:** small weak text on the left (full text on hover), view controls on the right.
- **Settings window:** `toolbar_tabs` at the top, a fixed 460 px width so it doesn't jump between
  tabs, and the window title set to the current tab name.

## Text style

British spelling ("Colour"). Title Case for buttons and menu items. An ellipsis when an action
needs more input ("Open SVG…", "Save PNG…"). Status messages are full sentences that say what
happened and, for failures, why.

## Before you finish a UI change

- Check light and dark mode (set Appearance in Settings).
- Check the narrowest window (720 px) and the smallest and largest card sizes.
- Check keyboard paths (Return to search, ⌘, ⌘O, ⌘+/−/0).
- If you couldn't see the result, say so in your summary.
