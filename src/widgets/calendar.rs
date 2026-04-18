use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Calendar, Label, Orientation, Window, glib};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::rc::Rc;
use crate::widgets::position::{load_positions, save_position};

const NAME: &str = "calendar";

pub fn spawn_calendar_widget() {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder()
        .title("cynager-calendar-widget")
        .build();

    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cynager-widget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.add_css_class("widget-box");

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("drag-handle");
    let handle_label = Label::new(Some("⠿ calendar"));
    handle_label.add_css_class("handle-label");
    handle.append(&handle_label);
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

    let outer_jig = outer.clone();

    gesture.connect_drag_begin(move |_, _, _| {
        outer_jig.add_css_class("jiggling");

        let outer_ref = outer_jig.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(360),
            move || { outer_ref.remove_css_class("jiggling"); }
        );
    });

    gesture.connect_drag_update(move |_, dx, dy| {
        let new_x = (cx.get() as f64 + dx).max(0.0) as i32;
        let new_y = (cy.get() as f64 + dy).max(0.0) as i32;
        win_drag.set_margin(Edge::Left, new_x);
        win_drag.set_margin(Edge::Top, new_y);
    });

    let cx2 = cur_x.clone();
    let cy2 = cur_y.clone();
    let outer_drop = outer.clone();
    gesture.connect_drag_end(move |_, dx, dy| {
        let new_x = (cx2.get() as f64 + dx).max(0.0) as i32;
        let new_y = (cy2.get() as f64 + dy).max(0.0) as i32;
        cx2.set(new_x);
        cy2.set(new_y);
        save_position(NAME, new_x, new_y);
        outer_drop.add_css_class("jiggling");
        let outer_ref = outer_drop.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(360),
            move || { outer_ref.remove_css_class("jiggling"); }
        );
    });

    handle.add_controller(gesture);
}