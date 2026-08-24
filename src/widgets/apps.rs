use gtk4::{Box as GtkBox, Button, Grid, Image, Orientation, Window, prelude::*, subclass::window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use std::fs as stdfs;
use std::path::Path as stdpath;
use crate::widgets::position::{load_positions, save_position};
use std::io::BufRead;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, EventKind};
use std::time::{Duration, Instant};
use async_channel::Sender;

const NAME: &str = "appd";

// still have to add drag and drop to create new .desktop and also right click menu to delete

#[derive(Debug, Clone)]
struct Applications {
    name: String,
    icon: Option<String>,
    exec: String,
}

fn sanitize_exec(exec_str: &str) -> String {
    exec_str
        .split_whitespace()
        .filter(|arg| !arg.starts_with('%'))
        .collect::<Vec<&str>>()
        .join(" ")
}

fn populate_repopulate() -> Vec<Applications> {
    let mut apps: Vec<Applications> = vec![];
    
    if let Some(desktop) = dirs::desktop_dir() {
        match stdfs::read_dir(desktop) {
            Ok(files) => {
                for file in files.flatten() {
                    let path = file.path();
                    if path.is_file() {
                        if path.extension().and_then(|ext| ext.to_str()) != Some("desktop") {
                            continue;
                        }
                        if let Ok(filehandle) = stdfs::File::open(&path) {
                            let reader = std::io::BufReader::new(filehandle);
                            let mut name = None;
                            let mut icon = None;
                            let mut exec = None;
                            let mut is_desktop_entry = false;
                            let mut nodisplay = false;

                            for line in reader.lines().flatten() {
                                let line = line.trim();

                                if line.starts_with('[') && line.ends_with(']') {
                                    is_desktop_entry = line == "[Desktop Entry]";
                                    continue;
                                }

                                if !is_desktop_entry || line.starts_with('#') || !line.contains('=') {
                                    continue;
                                }

                                let mut parts = line.splitn(2, '=');
                                if let (Some(key), Some(value)) = (parts.next(), parts.next()) {
                                    match key.trim() {
                                        "Name" if name.is_none() => name = Some(value.trim().to_string()),
                                        "Exec" => exec = Some(sanitize_exec(value.trim())),
                                        "Icon" => icon = Some(value.trim().to_string()),
                                        "NoDisplay" if value.trim() == "true" => nodisplay = true,
                                        _ => {}
                                    }
                                }
                            }

                            if !nodisplay {
                                if let (Some(name), Some(exec)) = (name, exec) {
                                    apps.push(Applications { name, icon, exec });
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => eprintln!("[appd] cannot read desktop directory: {}", e),
        }
    }

    apps
}

fn build_app_grid(grid: &Grid, apps: Vec<Applications>) {
    const ROWS: i32 = 2;

    for (idx, app) in apps.into_iter().enumerate() {
        let idx = idx as i32;
        let col = idx / ROWS;
        let row = idx % ROWS;

        let item = Button::new();
        item.set_halign(gtk4::Align::Center);
        item.add_css_class("appItem");

        let icon = Image::new();
        icon.set_pixel_size(48);
        match &app.icon {
            Some(icon_name) if stdpath::new(icon_name).is_absolute() => {
                icon.set_from_file(Some(icon_name));
            }
            Some(icon_name) => {
                icon.set_icon_name(Some(icon_name));
            }
            None => {
                icon.set_icon_name(Some("application-x-executable"));
            }
        }

        item.set_has_tooltip(true);
        item.set_tooltip_text(Some(&app.name));

        item.set_child(Some(&icon));

        let exec = app.exec.clone();
        item.connect_clicked(move |_| {
            let mut parts = exec.split_whitespace();
            if let Some(cmd) = parts.next() {
                let _ = std::process::Command::new(cmd).args(parts).spawn();
            }
        });

        grid.attach(&item, col, row, 1, 1);
    }
}

fn clear_grid(grid: &Grid) {
    while let Some(child) = grid.first_child() {
        grid.remove(&child);
    }
}

fn refresh_app_grid(grid: &Grid, win: &Window) {
    clear_grid(grid);
    build_app_grid(grid, populate_repopulate());
    win.set_visible(true);
}

fn watch_desktop_dir(desktop: std::path::PathBuf, tx: Sender<()>) {
    std::thread::spawn(move || {
        let (watch_tx, watch_rx) = std::sync::mpsc::channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(watch_tx) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[appd] failed to create watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&desktop, RecursiveMode::NonRecursive) {
            eprintln!("[appd] failed to watch desktop dir: {}", e);
            return;
        }

        for res in watch_rx {
            match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_)
                    ) {
                        if tx.send_blocking(()).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => eprintln!("[appd] watch error: {}", e),
            }
        }

        drop(watcher);
    });
}

pub fn spawn_appd_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder().title("capsuleWapd").build();
    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    // win.set_height_request(40);
    if let Some(m) = monitor {
        win.set_monitor(Some(m));
    }
    win.remove_css_class("background");

    let outer = GtkBox::new(Orientation::Horizontal, 10);
    outer.set_css_classes(&["starting", "outerAppd"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandle");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_hexpand(true);
    handle.set_vexpand(true);
    handle.set_halign(gtk4::Align::Center);

    let app_grid = Grid::builder()
        .column_spacing(5)
        .row_spacing(5)
        .build();

    build_app_grid(&app_grid, populate_repopulate());

    if let Some(desktop) = dirs::desktop_dir() {
        let (tx, rx) = async_channel::unbounded::<()>();
        watch_desktop_dir(desktop, tx);

        let grid_c = app_grid.clone();
        let win = win.clone();
        gtk4::glib::spawn_future_local(async move {
            let last_refresh: Cell<Option<Instant>> = Cell::new(None);
            let win = win.clone();
            while rx.recv().await.is_ok() {
                let now = Instant::now();
                let should_refresh = match last_refresh.get() {
                    Some(t) if now.duration_since(t) < Duration::from_millis(300) => false,
                    _ => true,
                };
                last_refresh.set(Some(now));
                if should_refresh {
                    win.set_visible(false);
                    refresh_app_grid(&grid_c, &win);
                }
            }
        });
    }

    outer.append(&handle);
    outer.append(&app_grid);

    win.set_child(Some(&outer));
    win.present();

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));
    let gesture = gtk4::GestureDrag::new();
    let outer_c = outer.clone();

    {
        let handle_c = handle.clone();
        gesture.connect_drag_begin(move |_, _, _| {
            outer_c.add_css_class("jiggling");
            handle_c.set_cursor_from_name(Some("grabbing"));
        });
    }
    {
        let cx = cur_x.clone();
        let cy = cur_y.clone();
        let win_c = win.clone();
        gesture.connect_drag_update(move |_, dx, dy| {
            let nx = (cx.get() as f64 + dx).max(0.0) as i32;
            let ny = (cy.get() as f64 + dy).max(0.0) as i32;
            win_c.set_margin(Edge::Left, nx);
            win_c.set_margin(Edge::Top, ny);
        });
    }
    {
        let cx2 = cur_x.clone();
        let cy2 = cur_y.clone();
        let outer_c = outer.clone();
        let handle_c = handle.clone();
        let win_c = win.clone();
        gesture.connect_drag_end(move |_, dx, dy| {
            handle_c.set_cursor_from_name(Some("grab"));
            let nx = (cx2.get() as f64 + dx).max(0.0) as i32;
            let ny = (cy2.get() as f64 + dy).max(0.0) as i32;
            cx2.set(nx);
            cy2.set(ny);
            win_c.set_margin(Edge::Left, nx);
            win_c.set_margin(Edge::Top, ny);
            save_position(NAME, nx, ny);
            outer_c.remove_css_class("jiggling");
        });
    }
    handle.add_controller(gesture);

    win
}