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

    let window = ApplicationWindow::builder()
        .application(app)
        .title("capsule")
        .build();

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

    window.init_layer_shell();
    window.set_layer(Layer::Top);
    for (edge, anchor) in [
        (Edge::Left, true),
        (Edge::Right, true),
        (Edge::Top, true),
        (Edge::Bottom, true),
    ] {
        window.set_anchor(edge, anchor);
    }

    let root = GtkBox::new(Orientation::Horizontal, 0);
    root.add_css_class("dock");

    let notif_btn = gtk4::Button::new();
    notif_btn.add_css_class("notif-btn");

    let badge = gtk4::Label::new(None);
    badge.add_css_class("notif-badge");
    badge.set_visible(true);

    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&notif_btn));
    overlay.add_overlay(&badge);
    badge.set_halign(gtk4::Align::End);
    badge.set_valign(gtk4::Align::Start);

    let osd_label = gtk4::Label::new(None);
    osd_label.add_css_class("osd-label");
 
    let osd_revealer = gtk4::Revealer::new();
    osd_revealer.set_transition_type(gtk4::RevealerTransitionType::Crossfade);
    osd_revealer.set_transition_duration(150);
    osd_revealer.set_child(Some(&osd_label));
    osd_revealer.set_reveal_child(true);

    root.append(&overlay);
    root.append(&osd_revealer);
    window.set_child(Some(&root));

    notifications::connect_notifications_to_dock(rx, &notif_btn, &badge);
    osd::connect_osd_to_dock(&osd_label, &osd_revealer);

    window.present();
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}