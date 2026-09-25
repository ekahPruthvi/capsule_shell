# `src/main.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `WidgetConfig` — data structure declared in this module.
- `WindowRecord` — data structure declared in this module.
- `BatteryState` — data structure declared in this module.
- `ClippyItem` — data structure declared in this module.

### Enums

- `ClippyPayload` — enum declared in this module.

### Constants

- `PEEK` — module constant.
- `STEPS` — module constant.
- `TICK_MS` — module constant.
- `TILE` — module constant.
- `STEP` — module constant.
- `MAX_SHOWN` — module constant.

### Functions

#### `default`

**Signature (abridged):** `fn default() -> Self {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `parse_widget_config`

**Signature (abridged):** `fn parse_widget_config(path: &str) -> Option<WidgetConfig> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_probe_watcher`

**Signature (abridged):** `fn spawn_probe_watcher( probe_path: String, interval: Duration, ) -> std::sync::mpsc::Receiver<WidgetConfig> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `resolve_monitor`

**Signature (abridged):** `fn resolve_monitor( display: &gtk4::gdk::Display, connector: &str, ) -> Option<gtk4::gdk::Monitor> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `pin_to_monitor`

**Signature (abridged):** `fn pin_to_monitor(window: &ApplicationWindow, monitor: Option<&gtk4::gdk::Monitor>) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_focused_window_id`

**Signature (abridged):** `fn get_focused_window_id() -> Option<u64> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `send_action`

**Signature (abridged):** `fn send_action(action: Action) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_windows`

**Signature (abridged):** `fn get_windows() -> Vec<WindowRecord> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_focused_output_size`

**Signature (abridged):** `fn get_focused_output_size() -> Option<(f64, f64)> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `corner_hide_target`

**Signature (abridged):** `fn corner_hide_target( orig_x: f64, orig_y: f64, win_w: i32, win_h: i32, screen_w: f64, screen_h: f64, ) -> (f64, f64) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `animate_float_window`

**Signature (abridged):** `fn animate_float_window( win_id: u64, from_x: f64, from_y: f64, to_x: f64, to_y: f64, on_done: impl Fn() + 'static, ) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `network_icon_and_tip`

**Signature (abridged):** `fn network_icon_and_tip(state: &ctrl::NetworkState) -> (&'static str, String) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_battery_state`

**Signature (abridged):** `fn get_battery_state() -> Option<BatteryState> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `battery_icon`

**Signature (abridged):** `fn battery_icon(state: &BatteryState) -> &'static str {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `battery_tip`

**Signature (abridged):** `fn battery_tip(state: &BatteryState) -> String {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_battery_watcher`

**Signature (abridged):** `fn spawn_battery_watcher(interval: Duration) -> std::sync::mpsc::Receiver<Option<BatteryState>> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `coping_with`

**Signature (abridged):** `fn coping_with(app: &Application) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `icon_for_path`

**Signature (abridged):** `fn icon_for_path(path: &std::path::Path) -> String {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `mime_for_path`

**Signature (abridged):** `fn mime_for_path(path: &std::path::Path) -> &'static str {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `percent_decode`

**Signature (abridged):** `fn percent_decode(s: &str) -> String {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `icon_name_for_item`

**Signature (abridged):** `fn icon_name_for_item(item: &ClippyItem) -> String {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_master_drag_icon`

**Signature (abridged):** `fn build_master_drag_icon( display: &gtk4::gdk::Display, items: &[ClippyItem], ) -> Option<gtk4::gdk::Paintable> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_master_provider`

**Signature (abridged):** `fn build_master_provider(items: &[ClippyItem]) -> Option<gtk4::gdk::ContentProvider> {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `sync_clippy_master`

**Signature (abridged):** `fn sync_clippy_master( clippy: &GtkBox, items_box: &GtkBox, items: &Rc<RefCell<Vec<ClippyItem>>>, master: &Rc<RefCell<Option<Button>>>, ) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `add_link_to_clippy`

**Signature (abridged):** `fn add_link_to_clippy( clippy: &GtkBox, items_box: &GtkBox, items: &Rc<RefCell<Vec<ClippyItem>>>, master: &Rc<RefCell<Option<Button>>>, url: &str, ) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `add_text_to_clippy`

**Signature (abridged):** `fn add_text_to_clippy( clippy: &GtkBox, items_box: &GtkBox, items: &Rc<RefCell<Vec<ClippyItem>>>, master: &Rc<RefCell<Option<Button>>>, text: &str, ) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `add_file_to_clippy`

**Signature (abridged):** `fn add_file_to_clippy( clippy: &GtkBox, items_box: &GtkBox, items: &Rc<RefCell<Vec<ClippyItem>>>, master: &Rc<RefCell<Option<Button>>>, uri: &str, ) {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

#### `main`

**Signature (abridged):** `fn main() {`

**Role:** This function is part of the `main` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `ampm` — styling hook assigned by this module.
- `batBtn` — styling hook assigned by this module.
- `clippy` — styling hook assigned by this module.
- `clippyFileBtn` — styling hook assigned by this module.
- `cosIcon` — styling hook assigned by this module.
- `dockBox` — styling hook assigned by this module.
- `dockcum` — styling hook assigned by this module.
- `dummytxt` — styling hook assigned by this module.
- `netBtn` — styling hook assigned by this module.
- `nooclip` — styling hook assigned by this module.
- `notiScroller` — styling hook assigned by this module.
- `notificationWindow` — styling hook assigned by this module.
- `notification_badge` — styling hook assigned by this module.
- `osdBox` — styling hook assigned by this module.
- `osdCapsule` — styling hook assigned by this module.
- `osdLabel` — styling hook assigned by this module.
- `tNa` — styling hook assigned by this module.
- `timeCapsule` — styling hook assigned by this module.
- `timeWindow` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)