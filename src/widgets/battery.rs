use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, DrawingArea, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::fs;
use std::rc::Rc;
use std::thread;
use crate::widgets::position::{load_positions, save_position};

const NAME: &str = "battery";

fn read_battery() -> Option<(bool, u8)> {
    for bat in &["BAT0", "BAT1"] {
        let base = format!("/sys/class/power_supply/{bat}");
        if let Ok(raw) = fs::read_to_string(format!("{base}/status")) {
            let s = raw.trim();
            let charging = s == "Charging" || s == "Full";
            if let Ok(cap_raw) = fs::read_to_string(format!("{base}/capacity")) {
                if let Ok(cap) = cap_raw.trim().parse::<u8>() {
                    return Some((charging, cap));
                }
            }
        }
    }
    None
}


fn spawn_worker<T, W, D>(work: W, on_done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    use std::sync::{Arc, Mutex};
    let slot: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let slot_t = slot.clone();
    thread::spawn(move || {
        let result: T = work();
        if let Ok(mut g) = slot_t.lock() { *g = Some(result); }
    });
    let mut on_done = Some(on_done);
    gtk4::glib::idle_add_local(move || {
        if let Ok(mut g) = slot.try_lock() {
            if let Some(v) = g.take() {
                if let Some(f) = on_done.take() { f(v); }
                return gtk4::glib::ControlFlow::Break;
            }
        }
        gtk4::glib::ControlFlow::Continue
    });
}

fn make_battery_ring(
    capacity_rc: Rc<Cell<u8>>,
    charging_rc: Rc<Cell<bool>>,
) -> DrawingArea {
    let da = DrawingArea::new();
    da.set_content_width(40);
    da.set_content_height(40);

    da.set_draw_func(move |_, cr, w, h| {
        let cap      = capacity_rc.get();
        let charging = charging_rc.get();
        let w        = w as f64;
        let h        = h as f64;

        let (tr, tg, tb) = (0.22, 0.22, 0.24);
        let (fr, fg, fb) = if charging { (0.18, 0.85, 0.45) } else { (1.0, 1.0, 1.0) };
        let stroke_w = 2.5_f64;
        let pad = stroke_w / 2.0 + 1.5; 


        let rx = w / 2.0 - pad;   // half-width of the bounding rect
        let ry = h / 2.0 - pad;   // half-height  (= corner radius for a true capsule)
        let r  = ry;              // corner radius
        let cx = w / 2.0;
        let cy = h / 2.0;

        let sx = rx - r;
        let perim = std::f64::consts::PI * 2.0 * r + 4.0 * sx;

        let start_x = cx - sx;   // left of bottom-straight, going right
        let start_y = cy + r;

        cr.new_path();
        cr.move_to(start_x, start_y);
        // bottom straight →
        cr.line_to(cx + sx, cy + r);
        // right cap arc (bottom → top, counter-clockwise = outward bulge)
        cr.arc_negative(cx + sx, cy, r, std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2);
        // top straight ←
        cr.line_to(cx - sx, cy - r);
        // left cap arc (top → bottom, counter-clockwise = outward bulge)
        cr.arc_negative(cx - sx, cy, r, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        // close back to start
        cr.close_path();

        cr.set_source_rgba(tr, tg, tb, 0.6);
        cr.set_line_width(stroke_w);
        cr.set_line_cap(gtk4::cairo::LineCap::Round);
        let _ = cr.stroke_preserve();

        let fill_len = perim * (cap as f64 / 100.0);
        let gap_len  = perim - fill_len;

        cr.set_dash(&[fill_len, gap_len], 0.0);
        cr.set_source_rgb(fr, fg, fb);
        cr.set_line_width(stroke_w);
        cr.set_line_cap(gtk4::cairo::LineCap::Round);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);

        cr.new_path();
        cr.move_to(start_x, start_y);
        cr.line_to(cx + sx, cy + r);
        cr.arc_negative(cx + sx, cy, r, std::f64::consts::FRAC_PI_2, -std::f64::consts::FRAC_PI_2);
        cr.line_to(cx - sx, cy - r);
        cr.arc_negative(cx - sx, cy, r, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
        cr.close_path();

        cr.set_dash(&[fill_len, gap_len], 0.0);
        cr.set_source_rgba(fr, fg, fb, 0.10);
        cr.set_line_width(stroke_w + 2.0);
        cr.set_line_cap(gtk4::cairo::LineCap::Round);
        let _ = cr.stroke();
        cr.set_dash(&[], 0.0);

        let label = if charging {
            format!("")
        } else {
            format!("",)
        };
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.select_font_face(
            "Cantarell",
            gtk4::cairo::FontSlant::Normal,
            gtk4::cairo::FontWeight::Normal,
        );
        cr.set_font_size(12.0);
        let (tw, th) = cr.text_extents(&label)
            .map(|e| (e.width(), e.height()))
            .unwrap_or((0.0, 0.0));
        cr.move_to(cx - tw / 2.0, cy + th / 2.0);
        let _ = cr.show_text(&label);
    });

    da
}

pub fn spawn_bat_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder().title("capsuleWb").build();
    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    if let Some(m) = monitor {
        win.set_monitor(Some(m));
    }
    win.set_width_request(190);
    win.set_height_request(190);
    win.remove_css_class("background");
    win.add_css_class("batpage");

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_css_classes(&["starting", "outerBat"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandle");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_margin_top(20);
    handle.set_margin_bottom(10);
    handle.set_hexpand(true);
    handle.set_halign(gtk4::Align::End);

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);

    let next_btn = Button::with_label("");
    next_btn.add_css_class("handleNextBtn");
    next_btn.set_width_request(30);

    let bat_page = GtkBox::new(Orientation::Vertical, 0);
    bat_page.set_hexpand(true);
    bat_page.set_vexpand(true);
    bat_page.set_halign(gtk4::Align::Fill);
    bat_page.set_valign(gtk4::Align::Fill);
    // bat_page.set_margin_start(30);
    // bat_page.set_margin_end(30);

    let bat_cap_rc      = Rc::new(Cell::new(0u8));
    let bat_charging_rc = Rc::new(Cell::new(false));

    let ring_da = make_battery_ring(bat_cap_rc.clone(), bat_charging_rc.clone());
    ring_da.add_css_class("batring");
    // ring_da.set_vexpand(true);
    ring_da.set_halign(gtk4::Align::Start);
    ring_da.set_valign(gtk4::Align::Baseline);

    let label_box = GtkBox::new(Orientation::Horizontal, 0);
    label_box.set_vexpand(true);
    label_box.set_hexpand(true);

    let bat_label = Label::new(Some(""));
    bat_label.add_css_class("batLabel");
    bat_label.set_vexpand(true);
    bat_label.set_halign(gtk4::Align::Start);
    bat_label.set_valign(gtk4::Align::End);

    let per = Label::new(Some("%"));
    per.add_css_class("PercentLabel");
    per.set_valign(gtk4::Align::End);
    per.set_margin_bottom(6);

    label_box.append(&bat_label);
    label_box.append(&per);
    label_box.append(&handle);

    bat_page.append(&ring_da);
    bat_page.append(&label_box);

    outer.append(&bat_page);
    // outer.append(&handle);

    win.set_child(Some(&outer));
    win.present();

    {
        let bat_label_c     = bat_label.clone();
        let bat_cap_rc_c    = bat_cap_rc.clone();
        let bat_charging_rc_c = bat_charging_rc.clone();
        let ring_da_c       = ring_da.clone();

        let update_battery = move |result: Option<(bool, u8)>| {
            match result {
                Some((charging, cap)) => {
                    bat_cap_rc_c.set(cap);
                    bat_charging_rc_c.set(charging);
                    bat_label_c.set_label(&format!("{}",cap));
                    ring_da_c.queue_draw();
                }
                None => {
                    bat_label_c.set_label("No battery");
                }
            }
        };

        {
            let upd = update_battery.clone();
            spawn_worker(read_battery, upd);
        }

        {
            let bat_cap_rc2    = bat_cap_rc.clone();
            let bat_charging_rc2 = bat_charging_rc.clone();
            let ring_da2       = ring_da.clone();
            let bat_label2     = bat_label.clone();
            gtk4::glib::timeout_add_local(std::time::Duration::from_secs(10), move || {
                let bca = bat_cap_rc2.clone();
                let bch = bat_charging_rc2.clone();
                let rd  = ring_da2.clone();
                let bl  = bat_label2.clone();
                spawn_worker(read_battery, move |result| {
                    match result {
                        Some((charging, cap)) => {
                            bca.set(cap);
                            bch.set(charging);
                            bl.set_label(&format!("{}",cap));
                            rd.queue_draw();
                        }
                        None => { bl.set_label("No battery"); }
                    }
                });
                gtk4::glib::ControlFlow::Continue
            });
        }
    }

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));
    let gesture = gtk4::GestureDrag::new();

    {
        let outer_c  = bat_page.clone();
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
        let outer_c  = bat_page.clone();
        let handle_c = handle.clone();
        let win_c    = win.clone();
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