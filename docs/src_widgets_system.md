# `src/widgets/system.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `MusicState` — data structure declared in this module.

### Constants

- `NAME` — module constant.

### Functions

#### `pctl`

**Signature (abridged):** `fn pctl(args: &[&str]) -> Option<String> {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `active_player`

**Signature (abridged):** `fn active_player() -> Option<String> {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `fetch_music_state`

**Signature (abridged):** `fn fetch_music_state() -> Option<MusicState> {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `fire_playerctl`

**Signature (abridged):** `fn fire_playerctl(args: &'static [&'static str]) {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_worker`

**Signature (abridged):** `fn spawn_worker<T, W, D>(work: W, on_done: D) where T: Send + 'static, W: FnOnce() -> T + Send + 'static, D: FnOnce(T) + 'static, {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_sys_widget`

**Signature (abridged):** `pub fn spawn_sys_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `fetch_pixbuf_from_url`

**Signature (abridged):** `fn fetch_pixbuf_from_url(url: &str) -> Option<gtk4::gdk_pixbuf::Pixbuf> {`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

#### `apply_music_state`

**Signature (abridged):** `fn apply_music_state( state: Option<MusicState>, track_label: &Label, artist_label: &Label, art_canvas: &gtk4::DrawingArea, art_pixbuf: &Rc<std::cell::RefCell<Option<gtk4::gdk_pixbuf::Pixbuf>>>, play_btn: &Button, play_i`

**Role:** This function is part of the `system` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `MuicInfo` — styling hook assigned by this module.
- `MusicWidget` — styling hook assigned by this module.
- `albumArt` — styling hook assigned by this module.
- `albumArtspinn` — styling hook assigned by this module.
- `artistLabel` — styling hook assigned by this module.
- `dragHandleM` — styling hook assigned by this module.
- `jiggling` — styling hook assigned by this module.
- `mediaBtn` — styling hook assigned by this module.
- `starting` — styling hook assigned by this module.
- `trackLabel` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)