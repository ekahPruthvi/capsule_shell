# `src/ctrl.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `SoundState` — data structure declared in this module.
- `OutputDevice` — data structure declared in this module.
- `AppPlayback` — data structure declared in this module.
- `SliderRowHandles` — data structure declared in this module.
- `RadioConfig` — data structure declared in this module.
- `AppTileHandles` — data structure declared in this module.
- `NetworkHub` — data structure declared in this module.

### Enums

- `NetworkState` — enum declared in this module.

### Constants

- `APP_TILE_SCROLL_STEP` — module constant.
- `SLIDER_TOUCH_GUARD` — module constant.

### Functions

#### `get_volume_and_mute`

**Signature (abridged):** `fn get_volume_and_mute(target: &str) -> (u32, bool) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_sound_state`

**Signature (abridged):** `fn get_sound_state() -> SoundState {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `sound_icon`

**Signature (abridged):** `fn sound_icon(state: &SoundState) -> &'static str {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_pactl_event_pump`

**Signature (abridged):** `fn spawn_pactl_event_pump(keywords: &'static [&'static str]) -> std::sync::mpsc::Receiver<()> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `wait_for_event_batch`

**Signature (abridged):** `fn wait_for_event_batch(evt_rx: &std::sync::mpsc::Receiver<()>, fallback: Duration, debounce: Duration) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_sound_watcher`

**Signature (abridged):** `pub fn spawn_sound_watcher(fallback_interval: Duration) -> std::sync::mpsc::Receiver<SoundState> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `parse_pactl_volume_pct`

**Signature (abridged):** `fn parse_pactl_volume_pct(line: &str) -> u32 {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_output_devices`

**Signature (abridged):** `fn get_output_devices() -> Vec<OutputDevice> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_app_playbacks`

**Signature (abridged):** `fn get_app_playbacks() -> Vec<AppPlayback> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `set_output_volume`

**Signature (abridged):** `fn set_output_volume(sink_name: &str, pct: u32) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `set_default_sink`

**Signature (abridged):** `fn set_default_sink(sink_name: &str) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `set_app_volume`

**Signature (abridged):** `fn set_app_volume(sink_input_id: &str, pct: u32) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `set_master_mute`

**Signature (abridged):** `fn set_master_mute(mute: bool) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `set_mic_mute`

**Signature (abridged):** `fn set_mic_mute(mute: bool) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_sound_devices_watcher`

**Signature (abridged):** `pub fn spawn_sound_devices_watcher( fallback_interval: Duration, ) -> std::sync::mpsc::Receiver<(Vec<OutputDevice>, Vec<AppPlayback>)> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_sound_row`

**Signature (abridged):** `fn build_sound_row( id: String, label_text: String, volume: u32, is_default: bool, on_change: Rc<dyn Fn(&str, u32)>, radio_cfg: Option<RadioConfig>, ) -> SliderRowHandles {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_app_row`

**Signature (abridged):** `fn build_app_row( id: String, label_text: String, icon_name: String, volume: u32, on_change: Rc<dyn Fn(&str, u32)>, ) -> AppTileHandles {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `ensure_placeholder`

**Signature (abridged):** `fn ensure_placeholder( list_rc: &Rc<gtk4::ListBox>, placeholder: &Rc<RefCell<Option<gtk4::ListBoxRow>>>, text: &str, ) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `clear_placeholder`

**Signature (abridged):** `fn clear_placeholder(list_rc: &Rc<gtk4::ListBox>, placeholder: &Rc<RefCell<Option<gtk4::ListBoxRow>>>) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `ensure_app_placeholder`

**Signature (abridged):** `fn ensure_app_placeholder( list_rc: &Rc<gtk4::FlowBox>, placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>, text: &str, ) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `clear_app_placeholder`

**Signature (abridged):** `fn clear_app_placeholder(list_rc: &Rc<gtk4::FlowBox>, placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `update_output_rows`

**Signature (abridged):** `fn update_output_rows( list_rc: &Rc<gtk4::ListBox>, rows: &Rc<RefCell<HashMap<String, SliderRowHandles>>>, placeholder: &Rc<RefCell<Option<gtk4::ListBoxRow>>>, radio_group: &Rc<RefCell<Option<gtk4::CheckButton>>>, device`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `update_app_rows`

**Signature (abridged):** `fn update_app_rows( list_rc: &Rc<gtk4::FlowBox>, rows: &Rc<RefCell<HashMap<String, AppTileHandles>>>, placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>, apps: &[AppPlayback], ) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `wifi_soft_blocked`

**Signature (abridged):** `fn wifi_soft_blocked() -> bool {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `has_internet`

**Signature (abridged):** `fn has_internet() -> bool {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `wifi_ssid`

**Signature (abridged):** `fn wifi_ssid(iface: &str) -> Option<String> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_network_state`

**Signature (abridged):** `fn get_network_state() -> NetworkState {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `new`

**Signature (abridged):** `pub fn new(interval: Duration) -> Self {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get`

**Signature (abridged):** `fn get_volume_and_mute(target: &str) -> (u32, bool) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_wifi_scan`

**Signature (abridged):** `pub fn spawn_wifi_scan() -> std::sync::mpsc::Receiver<Vec<(String, String, bool)>> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `network_icon_and_tip`

**Signature (abridged):** `fn network_icon_and_tip(state: NetworkState) -> (&'static str, String, String) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `toggle_wifi_adapter`

**Signature (abridged):** `fn toggle_wifi_adapter(enable: bool) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_wifi_networks`

**Signature (abridged):** `fn get_wifi_networks() -> Vec<(String, String, bool)> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `render_network_rows`

**Signature (abridged):** `fn render_network_rows( net_list_rc: &Rc<gtk4::ListBox>, networks: Vec<(String, String, bool)>, netbtn: &Button, net_panel: &Rc<GtkBox>, net_expanded: &Rc<RefCell<bool>>, ) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_round_user_icon`

**Signature (abridged):** `fn build_round_user_icon(pixbuf: Pixbuf, size: i32) -> DrawingArea {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `parse_probe_ver_block`

**Signature (abridged):** `fn parse_probe_ver_block(path: &str) -> Vec<(String, String)> {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_ver_row`

**Signature (abridged):** `fn build_ver_row(name: &str, version: &str) -> gtk4::ListBoxRow {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `populate_ver_table`

**Signature (abridged):** `fn populate_ver_table(list_rc: &Rc<gtk4::ListBox>) {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `is_airplane`

**Signature (abridged):** `fn is_airplane() -> bool {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `is_dnd`

**Signature (abridged):** `fn is_dnd() -> bool {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `toggle_dnd`

**Signature (abridged):** `fn toggle_dnd() {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_ctrl_capsules`

**Signature (abridged):** `pub fn spawn_ctrl_capsules( app: &Application, overlay_open: Rc<RefCell<bool>>, net_hub: NetworkHub, ) -> ApplicationWindow {`

**Role:** This function is part of the `ctrl` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `appTile` — styling hook assigned by this module.
- `appTileIcon` — styling hook assigned by this module.
- `appTileRow` — styling hook assigned by this module.
- `appTileTop` — styling hook assigned by this module.
- `appsGrid` — styling hook assigned by this module.
- `closeBox` — styling hook assigned by this module.
- `ctrlBTNSbox` — styling hook assigned by this module.
- `ctrlBackdrop` — styling hook assigned by this module.
- `ctrlBtnL` — styling hook assigned by this module.
- `ctrlBtnS` — styling hook assigned by this module.
- `ctrlExpanded` — styling hook assigned by this module.
- `ctrlExpandedS` — styling hook assigned by this module.
- `ctrlOverlay` — styling hook assigned by this module.
- `ctrlPanel` — styling hook assigned by this module.
- `ctrlpanelList` — styling hook assigned by this module.
- `cynageOS` — styling hook assigned by this module.
- `dndicon` — styling hook assigned by this module.
- `dockBtn` — styling hook assigned by this module.
- `flyplane` — styling hook assigned by this module.
- `netBtnBody` — styling hook assigned by this module.
- `netBtnLabel` — styling hook assigned by this module.
- `netListConnected` — styling hook assigned by this module.
- `netListEmpty` — styling hook assigned by this module.
- `netListRow` — styling hook assigned by this module.
- `netListRowBtn` — styling hook assigned by this module.
- `netListSSID` — styling hook assigned by this module.
- `netListScroll` — styling hook assigned by this module.
- `netListSignal` — styling hook assigned by this module.
- `netPanelActions` — styling hook assigned by this module.
- `netPanelBtn` — styling hook assigned by this module.
- `netPanelSwitch` — styling hook assigned by this module.
- `soundApptxt` — styling hook assigned by this module.
- `soundListDefault` — styling hook assigned by this module.
- `soundListEmpty` — styling hook assigned by this module.
- `soundListName` — styling hook assigned by this module.
- `soundListRadio` — styling hook assigned by this module.
- `soundListRow` — styling hook assigned by this module.
- `soundListRowWrap` — styling hook assigned by this module.
- `soundListSlider` — styling hook assigned by this module.
- `soundListValue` — styling hook assigned by this module.
- `soundPanelActions` — styling hook assigned by this module.
- `soundPanelIcon` — styling hook assigned by this module.
- `soundPanelSwitch` — styling hook assigned by this module.
- `soundTabs` — styling hook assigned by this module.
- `spinner` — styling hook assigned by this module.
- `spinning` — styling hook assigned by this module.
- `startingOSD` — styling hook assigned by this module.
- `userHello` — styling hook assigned by this module.
- `userName` — styling hook assigned by this module.
- `userPanelActions` — styling hook assigned by this module.
- `userPanelIcon` — styling hook assigned by this module.
- `verList` — styling hook assigned by this module.
- `verListRow` — styling hook assigned by this module.
- `verListRowBox` — styling hook assigned by this module.
- `verListValue` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)