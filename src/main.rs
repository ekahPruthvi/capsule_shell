use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox,
    CssProvider, Orientation,
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
        .build();

    time_window.init_layer_shell();
    time_window.set_layer(Layer::Top);
    time_window.set_height_request(30);
    time_window.set_anchor(Edge::Top, true);

    let main_shell = GtkBox::new(Orientation::Horizontal, 0); 
    main_shell.add_css_class("mainShell");

    let time_capsule = GtkBox::new(Orientation::Horizontal, 5);
    time_capsule.add_css_class("timeCapsule");
    time_capsule.set_height_request(30);
    time_capsule.set_halign(gtk4::Align::Center);
    time_capsule.set_valign(gtk4::Align::Start);
    time_capsule.set_width_request(100);
    time_capsule.set_hexpand(false);

    time_window.set_child(Some(&time_capsule));

    // notifications::connect_notifications_to_dock(rx, &notif_btn, &badge);
    // osd::connect_osd_to_dock(&osd_label, &osd_revealer);

    time_window.present();
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}