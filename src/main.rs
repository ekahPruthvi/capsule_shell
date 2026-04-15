use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label, Orientation, glib, prelude::*
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::env;
use gtk4::gio::File;

mod notifications;
mod osd;

fn coping_with(app: &Application) {
    let rx = notifications::spawn_messaging_daemon();

    let css = CssProvider::new();
    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let css_path = format!("{}/.config/capsule/style.css", home_dir);
    let file = File::for_path(&css_path);
    css.load_from_file(&file);

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );


    let time_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleT")
        .css_name("timeWindow")
        .build();

    time_window.init_layer_shell();
    time_window.set_namespace(Some("TimeCapsule"));
    time_window.set_layer(Layer::Top);
    time_window.set_height_request(30);
    time_window.set_anchor(Edge::Top, true);
    time_window.set_exclusive_zone(0);

    let time_capsule = GtkBox::new(Orientation::Horizontal, 5);
    time_capsule.add_css_class("timeCapsule");
    time_capsule.set_halign(gtk4::Align::Center);
    time_capsule.set_valign(gtk4::Align::Start);
    time_capsule.set_width_request(100);

    let cos = Button::new();
    let cos_logo = Image::from_file("/var/lib/cynager/icons/cos.svg");
    cos_logo.set_icon_size(gtk4::IconSize::Large);
    cos.set_child(Some(&cos_logo));

    let badge = Label::builder()
        .css_name("notification_badge")
        .halign(gtk4::Align::Center)
        .visible(true)
        .label("")
        .build();
    badge.set_wrap(true);
    badge.set_max_width_chars(25);
    badge.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let osd_box = GtkBox::new(Orientation::Horizontal, 5);

    time_capsule.append(&cos);
    time_capsule.append(&badge);
    time_capsule.append(&osd_box);

    time_window.set_child(Some(&time_capsule));

    notifications::connect_notifications_to_dock(rx, &time_window, &cos_logo, &badge);
    // osd::connect_osd_to_dock(&osd_label, &osd_revealer);

    time_window.present();
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}