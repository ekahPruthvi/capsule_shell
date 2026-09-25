# `src/widgets/stick.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Constants

- `NAME` — module constant.
- `PROBE_PATH` — module constant.

### Functions

#### `parse_stick_image_path`

**Signature (abridged):** `fn parse_stick_image_path(probe_text: &str) -> Option<String> {`

**Role:** This function is part of the `stick` module. Review its body for the exact control flow, error handling, and side effects.

#### `reload_image`

**Signature (abridged):** `fn reload_image(img: &Image) {`

**Role:** This function is part of the `stick` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_stick_widget`

**Signature (abridged):** `pub fn spawn_stick_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {`

**Role:** This function is part of the `stick` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `batpage` — styling hook assigned by this module.
- `dragHandlestick` — styling hook assigned by this module.
- `jiggling` — styling hook assigned by this module.
- `starting` — styling hook assigned by this module.
- `stickerImage` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)