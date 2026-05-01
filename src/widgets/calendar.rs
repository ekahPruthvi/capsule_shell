use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Calendar, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use crate::widgets::position::{load_positions, save_position};

const NAME: &str = "calendar";

pub fn spawn_calendar_widget() -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder()
        .title("capsuleWc")
        .build();

    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    win.remove_css_class("background");

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.add_css_class("widgetBox");
    outer.set_css_classes(&["starting"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandle");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_margin_bottom(5);
    // handle.set_width_request(150);
    // handle.set_halign(gtk4::Align::Center);
    handle.set_margin_start(20);
    handle.set_margin_end(20);
    outer.append(&handle);

    let cal = Calendar::new();
    cal.add_css_class("widget-calendar");
    outer.append(&cal);

    win.set_child(Some(&outer));
    win.present();

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));

    let gesture = gtk4::GestureDrag::new();
    let win_drag = win.clone();
    let cx = cur_x.clone();
    let cy = cur_y.clone();

    let outer_jig = cal.clone();
    let handle_clone = handle.clone();

    gesture.connect_drag_begin(move |_, _, _| {
        outer_jig.add_css_class("jiggling");
        handle_clone.set_cursor_from_name(Some("grabbing"));
    });

    gesture.connect_drag_update(move |_, dx, dy| {
        let new_x = (cx.get() as f64 + dx).max(0.0) as i32;
        let new_y = (cy.get() as f64 + dy).max(0.0) as i32;
        win_drag.set_margin(Edge::Left, new_x);
        win_drag.set_margin(Edge::Top, new_y);
    });

    let cx2 = cur_x.clone();
    let cy2 = cur_y.clone();
    let outer_drop = cal.clone();
    let handle_clone = handle.clone();

    gesture.connect_drag_end(move |_, dx, dy| {
        handle_clone.set_cursor_from_name(Some("grab"));
        let new_x = (cx2.get() as f64 + dx).max(0.0) as i32;
        let new_y = (cy2.get() as f64 + dy).max(0.0) as i32;
        cx2.set(new_x);
        cy2.set(new_y);
        save_position(NAME, new_x, new_y);
        outer_drop.remove_css_class("jiggling");
    });

    handle.add_controller(gesture);

    win
}

pub fn kill(win: &Window) {
    win.close();
}
