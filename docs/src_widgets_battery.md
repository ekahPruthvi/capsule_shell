# `src/widgets/battery.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Constants

- `NAME` — module constant.

### Functions

#### `read_battery`

**Signature (abridged):** `fn read_battery() -> Option<(bool, u8)> {`

**Role:** This function is part of the `battery` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_worker`

**Signature (abridged):** `fn spawn_worker<T, W, D>(work: W, on_done: D) where T: Send + 'static, W: FnOnce() -> T + Send + 'static, D: FnOnce(T) + 'static, {`

**Role:** This function is part of the `battery` module. Review its body for the exact control flow, error handling, and side effects.

#### `make_battery_ring`

**Signature (abridged):** `fn make_battery_ring( capacity_rc: Rc<Cell<u8>>, charging_rc: Rc<Cell<bool>>, ) -> DrawingArea {`

**Role:** This function is part of the `battery` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_bat_widget`

**Signature (abridged):** `pub fn spawn_bat_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {`

**Role:** This function is part of the `battery` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `PercentLabel` — styling hook assigned by this module.
- `batLabel` — styling hook assigned by this module.
- `batpage` — styling hook assigned by this module.
- `batring` — styling hook assigned by this module.
- `dragHandle` — styling hook assigned by this module.
- `handleNextBtn` — styling hook assigned by this module.
- `jiggling` — styling hook assigned by this module.
- `starting` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)