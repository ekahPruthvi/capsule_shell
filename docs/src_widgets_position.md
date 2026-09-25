# `src/widgets/position.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Constants

- `DAT_PATH` — module constant.

### Functions

#### `load_positions`

**Signature (abridged):** `pub fn load_positions() -> HashMap<String, (i32, i32)> {`

**Role:** This function is part of the `position` module. Review its body for the exact control flow, error handling, and side effects.

#### `save_position`

**Signature (abridged):** `pub fn save_position(name: &str, x: i32, y: i32) {`

**Role:** This function is part of the `position` module. Review its body for the exact control flow, error handling, and side effects.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)