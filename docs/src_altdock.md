# `src/altdock.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `NiriWindow` — data structure declared in this module.
- `TrayItemData` — data structure declared in this module.
- `TrayMenuLayer` — data structure declared in this module.
- `TrayState` — data structure declared in this module.
- `DockState` — data structure declared in this module.

### Enums

- `TrayCommand` — enum declared in this module.

### Constants

- `OUT_STEPS` — module constant.
- `BACK_STEPS` — module constant.
- `TICK_MS` — module constant.

### Functions

#### `parse_window`

**Signature (abridged):** `fn parse_window(v: &Value) -> Option<NiriWindow> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `windows_sorted`

**Signature (abridged):** `fn windows_sorted(map: &HashMap<u64, NiriWindow>) -> Vec<NiriWindow> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_niri_windows_map`

**Signature (abridged):** `fn get_niri_windows_map() -> HashMap<u64, NiriWindow> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `focus_window`

**Signature (abridged):** `fn focus_window(id: u64) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_focused_window_id`

**Signature (abridged):** `fn get_focused_window_id() -> Option<u64> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_window_position`

**Signature (abridged):** `fn get_window_position(id: u64) -> Option<(f64, f64, bool)> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `send_action`

**Signature (abridged):** `fn send_action(action: Action) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `nudge_window`

**Signature (abridged):** `fn nudge_window(id: u64, offset_y: f64, on_done: impl Fn() + 'static) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `focus_window_animated`

**Signature (abridged):** `fn focus_window_animated(id: u64) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `preview_window`

**Signature (abridged):** `fn preview_window( id: u64, hover_ctrl: EventControllerMotion, prev_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>, committed: Rc<Cell<bool>>, ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_niri_watcher`

**Signature (abridged):** `pub fn spawn_niri_watcher() -> std::sync::mpsc::Receiver<Vec<NiriWindow>> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `desktop_for_app_id`

**Signature (abridged):** `fn desktop_for_app_id(app_id: &str) -> Option<std::path::PathBuf> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `send_tray_snapshot`

**Signature (abridged):** `fn send_tray_snapshot( tx: &std::sync::mpsc::Sender<HashMap<String, TrayItemData>>, client: &TrayClient, ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_tray_watcher`

**Signature (abridged):** `fn spawn_tray_watcher() -> ( std::sync::mpsc::Receiver<HashMap<String, TrayItemData>>, TrayCmdSender, ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `tray_icon_image`

**Signature (abridged):** `fn tray_icon_image(item: &StatusNotifierItem) -> Image {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `tray_item_tooltip`

**Signature (abridged):** `fn tray_item_tooltip(item: &StatusNotifierItem) -> String {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `new`

**Signature (abridged):** `fn new() -> Rc<Self> {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `close`

**Signature (abridged):** `fn close(&self) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `is_open_for`

**Signature (abridged):** `fn is_open_for(&self, address: &str) -> bool {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_menu_box`

**Signature (abridged):** `fn build_menu_box( items: &[TrayMenuItem], address: &str, menu_path: &str, cmd_tx: &TrayCmdSender, layer: &Rc<TrayMenuLayer>, ) -> GtkBox {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `show_tray_menu`

**Signature (abridged):** `fn show_tray_menu( _anchor: &Button, layer: &Rc<TrayMenuLayer>, address: &str, menu: &TrayMenu, menu_path: &str, cmd_tx: &TrayCmdSender, ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `make_tray_button`

**Signature (abridged):** `fn make_tray_button( address: String, layer: &Rc<TrayMenuLayer>, data_cell: Rc<RefCell<TrayItemData>>, cmd_tx: TrayCmdSender, ) -> Button {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `update_traybox`

**Signature (abridged):** `fn update_traybox( tray_box: &GtkBox, state: &Rc<RefCell<TrayState>>, items: &HashMap<String, TrayItemData>, cmd_tx: &TrayCmdSender, layer: &Rc<TrayMenuLayer>, ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `make_dock_btn`

**Signature (abridged):** `fn make_dock_btn(win: &NiriWindow) -> Button {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `update_dockbox`

**Signature (abridged):** `fn update_dockbox( dockbox: &GtkBox, tray_box: &GtkBox, tray_menu_layer: &GtkBox, state: &Rc<RefCell<DockState>>, windows: &[NiriWindow], ) {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_altdock`

**Signature (abridged):** `pub fn spawn_altdock(app: &Application, dockbox: GtkBox) -> ApplicationWindow {`

**Role:** This function is part of the `altdock` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `dockBtn` — styling hook assigned by this module.
- `dockBtnActive` — styling hook assigned by this module.
- `dockEmpty` — styling hook assigned by this module.
- `dockOverlay` — styling hook assigned by this module.
- `dockcum` — styling hook assigned by this module.
- `dockleave` — styling hook assigned by this module.
- `tray` — styling hook assigned by this module.
- `trayBtn` — styling hook assigned by this module.
- `trayMenuBox` — styling hook assigned by this module.
- `trayMenuItem` — styling hook assigned by this module.
- `trayMenuLayer` — styling hook assigned by this module.
- `trayMenuPopover` — styling hook assigned by this module.
- `trayMenuSeparator` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)