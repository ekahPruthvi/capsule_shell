use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label, Orientation, glib, prelude::*
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::{env, time::Duration};
use chrono::Local;
use gtk4::gio::File;

mod notifications;
mod osd;

fn makin_noti_window(app: &Application, boxxy: &gtk4::ScrolledWindow){
    let noti_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleN")
        .build();

    noti_window.init_layer_shell();
    noti_window.set_namespace(Some("Notifications"));
    noti_window.set_layer(Layer::Bottom);
    noti_window.set_height_request(100);
    noti_window.remove_css_class("background");
    noti_window.set_anchor(Edge::Bottom, true);
    noti_window.set_exclusive_zone(-1);

    noti_window.set_child(Some(boxxy));

    noti_window.present();
}

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
        .css_classes(["timeWindow"])
        .build();

    time_window.init_layer_shell();
    time_window.set_namespace(Some("TimeCapsule"));
    time_window.set_layer(Layer::Top);
    time_window.set_height_request(30);
    time_window.remove_css_class("background");
    time_window.set_anchor(Edge::Top, true);
    time_window.set_exclusive_zone(0);
    time_window.set_width_request(400);

    let time_capsule = GtkBox::new(Orientation::Horizontal, 5);
    time_capsule.set_css_classes(&["timeCapsule", "starting"]);
    time_capsule.set_halign(gtk4::Align::Center);
    time_capsule.set_valign(gtk4::Align::Start);
    time_capsule.set_hexpand(true);
    time_capsule.set_margin_top(5);
    time_capsule.set_margin_bottom(5);
    time_capsule.set_width_request(300);

    let timendate = GtkBox::new(Orientation::Horizontal, 5);
    let time = Label::new(Some(""));
    time.set_justify(gtk4::Justification::Center);
    let ampm = Label::new(Some("cynageOS"));
    ampm.set_css_classes(&["ampm"]);

    timendate.append(&time);
    timendate.append(&ampm);
    
    let time_and_actions = Button::builder()
        .css_classes(["tNa"])
        .child(&timendate)
        .hexpand(true)
        .halign(gtk4::Align::End)
        .build();

    let time_win = time_capsule.clone();
    let time_actual_window = time_window.clone();
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let now = Local::now();
        let time_str = now.format("%I:%M").to_string();
        
        time.set_text(&time_str);
        ampm.set_text(&now.format(" %p \n %a, %b %e").to_string());

        time_win.remove_css_class("starting");
        time_actual_window.set_width_request(300);
        glib::ControlFlow::Continue
    });

    let cos = Button::new();
    let cos_logo = Image::from_file("/var/lib/cynager/icons/cos.svg");
    cos_logo.set_icon_size(gtk4::IconSize::Large);
    cos.set_child(Some(&cos_logo));
    cos.set_css_classes(&["cosIcon"]);

    let badge = Label::builder()
        .css_classes(["notification_badge"])
        .halign(gtk4::Align::Center)
        .visible(false)
        .label("")
        .build();
    badge.set_wrap(true);
    badge.set_max_width_chars(500);
    badge.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    let osd_box = GtkBox::new(Orientation::Horizontal, 5);
    osd_box.set_hexpand(true);
    osd_box.set_halign(gtk4::Align::Center);

    let osd = GtkBox::new(Orientation::Horizontal, 5);
    osd.set_hexpand(false);
    osd.set_halign(gtk4::Align::Start);

 
    let osd_revealer = gtk4::Revealer::new();
    osd_revealer.set_transition_type(gtk4::RevealerTransitionType::Crossfade);
    osd_revealer.set_transition_duration(150);
    osd_revealer.set_child(Some(&osd));
    osd_revealer.set_reveal_child(false);
    osd_revealer.set_css_classes(&["osdBox"]);
    osd_revealer.set_width_request(300);
    osd_revealer.set_visible(false);


    osd_box.append(&osd_revealer);

    time_capsule.append(&cos);
    time_capsule.append(&badge);
    time_capsule.append(&osd_box);
    time_capsule.append(&time_and_actions);

    time_window.set_child(Some(&time_capsule));


    let noti_boxy_inner_notifications_all = GtkBox::new(Orientation::Horizontal, 0);

    notifications::connect_notifications_to_dock(rx, &time_capsule, &time_window, &cos_logo, &cos, &badge, &noti_boxy_inner_notifications_all);
    osd::connect_osd_to_dock(&osd, &osd_revealer, &time_capsule, &time_window);

    time_window.present();

    let noti_boxy = GtkBox::new(Orientation::Vertical, 0);
    noti_boxy.append(&noti_boxy_inner_notifications_all);
    noti_boxy.set_css_classes(&["notificationWindow"]);
    noti_boxy.set_margin_bottom(10);
    noti_boxy.set_width_request(300);
    noti_boxy.set_halign(gtk4::Align::Center);
    let display = gtk4::gdk::Display::default().expect("Could not get default display");
    let monitors = display.monitors();

    let scrolled_window = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)               
        .child(&noti_boxy)                        
        .build();

    if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
        let geometry = monitor.geometry();
        let width = geometry.width();
        scrolled_window.set_width_request(width);
    }
    

    let appy = app.clone();
    // time_and_actions.connect_clicked( move |_| {
        makin_noti_window(&appy, &scrolled_window);
    // });
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}