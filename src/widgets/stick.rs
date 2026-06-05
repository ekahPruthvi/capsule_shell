use gtk4::{prelude::*, Box as GtkBox, Image, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use crate::widgets::position::{load_positions, save_position};

const NAME: &str = "sticker";
const PROBE_PATH: &str = "/var/lib/cynager/info.probe";

fn parse_stick_image_path(probe_text: &str) -> Option<String> {
    for line in probe_text.lines() {
        let line = line.trim();
        let line = line.strip_prefix(':').unwrap_or(line);
        if let Some(rest) = line.strip_prefix("sticker") {
            let rest = rest.trim();
            if let Some(path) = rest.strip_prefix(':') {
                let path = path.trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }
    None
}

fn reload_image(img: &Image) {
    let probe_text = match std::fs::read_to_string(PROBE_PATH) {
        Ok(t) => t,
        Err(_) => return,
    };
    if let Some(path) = parse_stick_image_path(&probe_text) {
        match gtk4::gdk_pixbuf::Pixbuf::from_file_at_scale(&path, -1, 200, true) {
            Ok(pb) => {
                let w = pb.width();
                let texture = gtk4::gdk::Texture::for_pixbuf(&pb);
                img.set_paintable(Some(&texture));
                img.set_size_request(w, 200);
            }
            Err(e) => {
                eprintln!("[stick] failed to load image {path}: {e}");
                img.set_paintable(None::<&gtk4::gdk::Paintable>);
            }
        }
    } else {
        img.set_paintable(None::<&gtk4::gdk::Paintable>);
    }
}

pub fn spawn_stick_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder().title("capsuleWst").build();
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
    // outer.set_css_classes(&["starting"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandlestick");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_hexpand(true);
    handle.set_vexpand(true);
    handle.set_halign(gtk4::Align::Center);

    let sticker_img = Image::new();
    sticker_img.set_pixel_size(180);
    sticker_img.set_size_request(-1, 200);
    sticker_img.add_css_class("stickerImage");
    handle.append(&sticker_img);
    sticker_img.set_vexpand(false);
    sticker_img.set_hexpand(false);

    reload_image(&sticker_img);

    {
        let img_ref = sticker_img.clone();
        let probe_file = gtk4::gio::File::for_path(PROBE_PATH);
        if let Ok(monitor) = probe_file.monitor_file(
            gtk4::gio::FileMonitorFlags::NONE,
            gtk4::gio::Cancellable::NONE,
        ) {
            monitor.connect_changed(move |_mon, _file, _other, event| {
                use gtk4::gio::FileMonitorEvent;
                match event {
                    FileMonitorEvent::Changed
                    | FileMonitorEvent::ChangesDoneHint
                    | FileMonitorEvent::MovedIn
                    | FileMonitorEvent::Renamed => {
                        reload_image(&img_ref);
                    }
                    _ => {}
                }
            });
            unsafe {
                outer.set_data("_probe_monitor", monitor);
            }
        }
    }

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