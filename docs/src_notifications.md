# `src/notifications.rs`

This page documents the module's current implementation.

## Responsibilities

- Defines the functions listed below and coordinates GTK widgets, system APIs, persistence, or compositor integration as applicable.
- Uses the GTK main thread for UI mutations; blocking or external-command work may be delegated to worker threads.

## Public and private API

### Structs

- `Notification` — data structure declared in this module.
- `LastNotiGroup` — data structure declared in this module.
- `NotificationServer` — data structure declared in this module.

### Functions

#### `notify`

**Signature (abridged):** `async fn notify( &self, app_name: &str, _replaces_id: u32, app_icon: &str, summary: &str, body: &str, _actions: Vec<String>, hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>, _expire_timeout: i32, ) -`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_capabilities`

**Signature (abridged):** `async fn get_capabilities(&self) -> Vec<String> {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `get_server_information`

**Signature (abridged):** `async fn get_server_information(&self) -> (&str, &str, &str, &str) {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `close_notification`

**Signature (abridged):** `async fn close_notification(&self, _id: u32) {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `is_it_dnd`

**Signature (abridged):** `fn is_it_dnd() -> String {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `spawn_messaging_daemon`

**Signature (abridged):** `pub fn spawn_messaging_daemon() -> UnboundedReceiver<Notification> {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `play_notification_sound`

**Signature (abridged):** `fn play_notification_sound() {`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

#### `connect_notifications_to_dock`

**Signature (abridged):** `pub fn connect_notifications_to_dock( mut rx: UnboundedReceiver<Notification>, noti_window: &GtkBox, main_window: &ApplicationWindow, app_img: &Image, cos_btn: &Button, badge: &Label, badge_head: &Label, noti_all: &GtkBo`

**Role:** This function is part of the `notifications` module. Review its body for the exact control flow, error handling, and side effects.

## CSS classes referenced

The module references these CSS classes directly from Rust:

- `appName` — styling hook assigned by this module.
- `blip` — styling hook assigned by this module.
- `clearBtn` — styling hook assigned by this module.
- `deleteBtn` — styling hook assigned by this module.
- `notiIcon` — styling hook assigned by this module.
- `notiPopup` — styling hook assigned by this module.
- `notiPopupBox` — styling hook assigned by this module.
- `notificationAll` — styling hook assigned by this module.
- `notificationAllLabelBody` — styling hook assigned by this module.
- `notificationAllLabelSummary` — styling hook assigned by this module.
- `popupanim` — styling hook assigned by this module.
- `spinning-coin` — styling hook assigned by this module.
- `timeCapsule` — styling hook assigned by this module.
- `vanish` — styling hook assigned by this module.

## Integration notes

- UI objects must be created and updated on the GTK thread.
- External files and commands may be unavailable; callers should preserve the module's existing fallback behavior.
- When changing this module, update the corresponding CSS and any links in the main README if responsibilities change.

## Related pages

- [Main README](../README.md)
- [Theme reference](../theme.md)