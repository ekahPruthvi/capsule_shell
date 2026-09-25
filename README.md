# Capsule Shell Documentation

This documentation is generated from the current Rust source tree and CSS files in this repository. It is organized by implementation file so each module can be read independently.

## Documentation map

| Area | Documentation | Responsibility |
|---|---|---|
| `src/altdock.rs` | [ src_altdock.md ](./docs/src_altdock.md) | Rust module documentation. |
| `src/ctrl.rs` | [ src_ctrl.md ](./docs/src_ctrl.md) | Rust module documentation. |
| `src/main.rs` | [ src_main.md ](./docs/src_main.md) | Rust module documentation. |
| `src/notifications.rs` | [ src_notifications.md ](./docs/src_notifications.md) | Rust module documentation. |
| `src/osd.rs` | [ src_osd.md ](./docs/src_osd.md) | Rust module documentation. |
| `src/ssd.rs` | [ src_ssd.md ](./docs/src_ssd.md) | Rust module documentation. |
| `src/widgets/appd.rs` | [ src_widgets_appd.md ](./docs/src_widgets_appd.md) | Rust module documentation. |
| `src/widgets/battery.rs` | [ src_widgets_battery.md ](./docs/src_widgets_battery.md) | Rust module documentation. |
| `src/widgets/calendar.rs` | [ src_widgets_calendar.md ](./docs/src_widgets_calendar.md) | Rust module documentation. |
| `src/widgets/mod.rs` | [ src_widgets_mod.md ](./docs/src_widgets_mod.md) | Rust module documentation. |
| `src/widgets/position.rs` | [ src_widgets_position.md ](./docs/src_widgets_position.md) | Rust module documentation. |
| `src/widgets/stick.rs` | [ src_widgets_stick.md ](./docs/src_widgets_stick.md) | Rust module documentation. |
| `src/widgets/system.rs` | [ src_widgets_system.md ](./docs/src_widgets_system.md) | Rust module documentation. |
| Theme | [theme.md](./theme.md) | CSS class catalogue and component styling map. |

## Project structure

```text
src/
├── main.rs              # Application entry point and orchestration
├── ctrl.rs              # Control center
├── osd.rs               # Volume/brightness OSD
├── notifications.rs     # Notification service
├── ssd.rs               # Side decorations / Niri geometry
├── altdock.rs           # Dock and tray
└── widgets/             # Independent desktop widgets
    ├── appd.rs
    ├── battery.rs
    ├── calendar.rs
    ├── position.rs
    ├── stick.rs
    ├── system.rs
    └── mod.rs
config/capsule/          # Light and dark GTK CSS
```

## Runtime concepts

- **GTK4 + layer-shell:** windows are presented as desktop overlays and anchored to compositor edges.
- **Niri integration:** selected modules query or control windows through Niri actions and events.
- **Persistent positions:** widget coordinates are stored in `/var/lib/cynager/desktop/widgets.dat`.
- **CSS-driven appearance:** Rust assigns CSS classes while `config/capsule/light.css` and `dark.css` provide styling.
- **Background work:** potentially blocking operations are moved to worker threads and returned to the GTK main loop where appropriate.

## Build and run

1. Install the system dependencies required by GTK4, gtk4-layer-shell, Cairo/Pixbuf, playerctl, and the compositor integration used by the project.
2. Review `Cargo.toml` and your compositor/session environment.
3. Build with `cargo build` and run with `cargo run`.
4. Keep the application running inside the graphical Wayland session for layer-shell and monitor APIs to work.

## Notes

The pages describe behavior visible in the current source. Any external commands, filesystem paths, compositor APIs, or desktop-entry formats are runtime dependencies and should be validated on the target system.