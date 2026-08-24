use gtk4::{Box as GtkBox, Grid, Image, Orientation, Window, prelude::*};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use std::fs as stdfs;
use std::path::Path as stdpath;
use crate::widgets::position::{load_positions, save_position};
use std::io::BufRead;

const NAME: &str = "appd";

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

        let item = GtkBox::new(Orientation::Vertical, 4);
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

        let label = gtk4::Label::new(Some(&app.name));
        label.set_max_width_chars(10);
        label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        label.add_css_class("appLabel");

        item.append(&icon);
        item.append(&label);

        // Launch the app on click.
        let exec = app.exec.clone();
        let click = gtk4::GestureClick::new();
        click.connect_released(move |_, _, _, _| {
            let mut parts = exec.split_whitespace();
            if let Some(cmd) = parts.next() {
                let _ = std::process::Command::new(cmd).args(parts).spawn();
            }
        });
        item.add_controller(click);

        grid.attach(&item, col, row, 1, 1);
    }
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
    win.set_height_request(240);
    if let Some(m) = monitor {
        win.set_monitor(Some(m));
    }
    win.remove_css_class("background");
    win.add_css_class("batpage");

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_css_classes(&["starting"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandlestick");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_hexpand(true);
    handle.set_vexpand(true);
    handle.set_halign(gtk4::Align::Center);

    let app_grid = Grid::builder()
        .column_spacing(5)
        .row_spacing(5)
        .build();

    handle.append(&app_grid);

    build_app_grid(&app_grid, populate_repopulate());
    
    outer.append(&handle);

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