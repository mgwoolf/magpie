# egui pitfalls

egui 0.33 behaviours that caused bugs or surprises in this project. Check here before fighting
the framework.

## Hit testing: later widgets win

When interactive rects overlap, the widget registered **later** in the frame gets the click.

- To make a background drag area (title bar), register it **first**: `titlebar_drag` is called
  before the toolbar's widgets.
- Don't add a "click anywhere to focus" rect *after* a `TextEdit`. It will swallow clicks meant
  for the text and the clear button (a bug we removed from `search_field`).

## `with_layout` and cross-axis alignment

`ui.with_layout(Layout::right_to_left(Align::Center), …)` inside a vertical layout centres its
content in **all remaining height**, so a button can land halfway down the sidebar. Use
`Align::Min` there (see `sidebar_search`).

## Panel separators turn black on hover

`SidePanel`'s built-in resize handle draws its line with `widgets.hovered.fg_stroke` (the
**text** colour) while hovered or dragged. There's no style knob for it. Magpie uses a
fixed-width panel (`resizable(false)`, `show_separator_line(false)`) and draws its own divider
(`theme::sidebar_divider`) in the **background layer**, so menus and windows stay above it.

## Selection colours are shared

`visuals.selection.stroke` is the text-edit focus ring, the text colour of selected labels, *and*
the text colour inside text selections. `selection.bg_fill` is both the selected-row background
and the text-selection highlight. So the accent goes in the stroke and a translucent accent in
the fill.

## ComboBox closes on any click

By default a `ComboBox` popup closes when you click inside it, so a filter `TextEdit` in the popup
is unusable. Use `.close_behavior(PopupCloseBehavior::CloseOnClickOutside)` (see
`icon_set_picker`).

## ⌘+ / ⌘− zoom the whole UI

egui zooms the UI on ⌘+/− by default. Magpie sets `options.zoom_with_keyboard = false` and uses
those keys (and `zoom_delta()` from pinch) to resize the result grid instead.

## Fonts: variable fonts load one weight

ab_glyph loads `SFNS.ttf` at its default (regular) instance. There's no bold:
`RichText::strong()` only changes colour. Use size or colour for hierarchy.

## macOS window chrome

- `with_fullsize_content_view(true)` + `with_titlebar_shown(false)` puts content under the
  transparent title bar. Reserve `TITLEBAR_H` (28 px) at the top for the traffic lights.
- The native title is hidden (`with_title_shown(false)`) and drawn by `theme::window_title`.
  The native title isn't reliably centred over custom content, and we want it over the full
  window. `with_title` still sets the name used by Mission Control and the Window menu.
- `ViewportCommand::SetTheme` switches the native chrome to match the chosen `Appearance`.
- Window dragging uses `ViewportCommand::StartDrag` on `drag_started`; zoom on double-click via
  `ViewportCommand::Maximized`.

## Widget IDs

Two widgets with the same `id_salt` under the same parent `Ui` clash (egui shows a red warning).
Salt IDs with something unique, e.g. `ui.id().with(("segment", i))`.

## Agent editing tip

`cargo fmt` reflows long lines. If you edit by exact string matching (sed, Python replace), re-read
the file after formatting, because a match that worked before `cargo fmt` may fail after.
