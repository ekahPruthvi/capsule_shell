# `src/ssd.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `FocusedGeo` — data structure declared in this module.

### Enums

- `SsdEvent` — enum declared in this module.

### Constants

- `DRAG_THRESHOLD` — module constant.

### Functions

#### `spawn_shelly_side_decorations`

**Signature (abridged):** `pub fn spawn_shelly_side_decorations(app: &gtk4::Application) {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `ghost_box_set_size`

**Signature (abridged):** `fn ghost_box_set_size(ghost_win: &ApplicationWindow, width: i32, height: i32) {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `ghost_box_set_icon`

**Signature (abridged):** `fn ghost_box_set_icon(ghost_win: &ApplicationWindow, app_id: Option<&str>) {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `monitor_at_point`

**Signature (abridged):** `fn monitor_at_point(x: i32, y: i32) -> Option<gdk::Monitor> {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `find_monitor_by_connector`

**Signature (abridged):** `fn find_monitor_by_connector(connector: &str) -> Option<gdk::Monitor> {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `make_btn`

**Signature (abridged):** `fn make_btn(label: &str, classes: &[&str]) -> Button {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `niri_action`

**Signature (abridged):** `fn niri_action(action: Action) {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `niri_event_loop`

**Signature (abridged):** `fn niri_event_loop(tx: mpsc::Sender<SsdEvent>) {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

#### `query_focused_geo`

**Signature (abridged):** `fn query_focused_geo() -> Option<FocusedGeo> {`

**Role:** This function is part of the `ssd` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `ampm` — styling hook assigned by this module.
- `ssdBar` — styling hook assigned by this module.
- `ssdDragGhost` — styling hook assigned by this module.
- `ssdDragGhostIcon` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)