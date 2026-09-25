# `src/widgets/appd.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `Applications` — data structure declared in this module.

### Constants

- `NAME` — module constant.
- `ROWS` — module constant.

### Functions

#### `sanitize_exec`

**Signature (abridged):** `fn sanitize_exec(exec_str: &str) -> String {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `populate_repopulate`

**Signature (abridged):** `fn populate_repopulate() -> Vec<Applications> {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `build_app_grid`

**Signature (abridged):** `fn build_app_grid(grid: &Grid, apps: Vec<Applications>) {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `clear_grid`

**Signature (abridged):** `fn clear_grid(grid: &Grid) {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `refresh_app_grid`

**Signature (abridged):** `fn refresh_app_grid(grid: &Grid, win: &Window) {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `watch_desktop_dir`

**Signature (abridged):** `fn watch_desktop_dir(desktop: std::path::PathBuf, tx: Sender<()>) {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_appd_widget`

**Signature (abridged):** `pub fn spawn_appd_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {`

**Role:** This function is part of the `appd` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `appdBtn` — styling hook assigned by this module.
- `appdMenu` — styling hook assigned by this module.
- `dockBtn` — styling hook assigned by this module.
- `dragHandle` — styling hook assigned by this module.
- `errWidget` — styling hook assigned by this module.
- `jiggling` — styling hook assigned by this module.
- `okWidget` — styling hook assigned by this module.
- `outerAppd` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)