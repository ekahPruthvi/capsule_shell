# `src/osd.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `AudioState` — data structure declared in this module.

### Enums

- `OsdEvent` — enum declared in this module.

### Functions

#### `adjust_volume`

**Signature (abridged):** `fn adjust_volume(delta: f64) {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `adjust_brightness`

**Signature (abridged):** `fn adjust_brightness(delta: f64) {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `find_backlight`

**Signature (abridged):** `fn find_backlight() -> Option<(std::path::PathBuf, u64)> {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `read_brightness_percent`

**Signature (abridged):** `fn read_brightness_percent(path: &std::path::Path, max: u64) -> Option<u32> {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_brightness_watcher`

**Signature (abridged):** `fn spawn_brightness_watcher() -> Option<std_mpsc::Receiver<u32>> {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `connect_brightness`

**Signature (abridged):** `fn connect_brightness( rx: std_mpsc::Receiver<u32>, osd_box: gtk4::Box, revealer: gtk4::Revealer, capsule: gtk4::Box, window: gtk4::ApplicationWindow, hide_id: Rc<RefCell<Option<glib::SourceId>>>, osd_label: gtk4::Label,`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `connect_osd_to_dock`

**Signature (abridged):** `pub fn connect_osd_to_dock( osd_box: &gtk4::Box, osd_revealer: &gtk4::Revealer, capsule: &gtk4::Box, window: &gtk4::ApplicationWindow, osd_label: &gtk4::Label, ) {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `on_context_ready`

**Signature (abridged):** `fn on_context_ready( ctx: &Rc<RefCell<Context>>, state: &Rc<RefCell<AudioState>>, osd_box: &gtk4::Box, revealer: &gtk4::Revealer, capsule: &gtk4::Box, window: &gtk4::ApplicationWindow, hide_id: &Rc<RefCell<Option<glib::S`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `fetch_sink_info`

**Signature (abridged):** `fn fetch_sink_info( ctx: &Rc<RefCell<Context>>, state: &Rc<RefCell<AudioState>>, osd_box: &gtk4::Box, revealer: &gtk4::Revealer, capsule: &gtk4::Box, window: &gtk4::ApplicationWindow, hide_id: &Rc<RefCell<Option<glib::So`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `fetch_source_info`

**Signature (abridged):** `fn fetch_source_info( ctx: &Rc<RefCell<Context>>, state: &Rc<RefCell<AudioState>>, osd_box: &gtk4::Box, revealer: &gtk4::Revealer, capsule: &gtk4::Box, window: &gtk4::ApplicationWindow, hide_id: &Rc<RefCell<Option<glib::`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `show_osd`

**Signature (abridged):** `fn show_osd( osd_box: &gtk4::Box, revealer: &gtk4::Revealer, capsule: &gtk4::Box, window: &gtk4::ApplicationWindow, hide_id: &Rc<RefCell<Option<glib::SourceId>>>, event: OsdEvent, osd_label: &gtk4::Label, ) {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `apply_osd_event`

**Signature (abridged):** `fn apply_osd_event(osd_box: &gtk4::Box, event: &OsdEvent, osd_label: &gtk4::Label) {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

#### `pa_vol_to_percent`

**Signature (abridged):** `fn pa_vol_to_percent(v: Volume) -> u32 {`

**Role:** This function is part of the `osd` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `osd-brightness` — styling hook assigned by this module.
- `osd-hide` — styling hook assigned by this module.
- `osd-mic` — styling hook assigned by this module.
- `osd-muted` — styling hook assigned by this module.
- `osd-show` — styling hook assigned by this module.
- `osd-volume` — styling hook assigned by this module.
- `scrollPad` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)