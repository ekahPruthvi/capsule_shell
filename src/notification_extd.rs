use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, Button, Image
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::env;
use gtk4::gio::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use glib::ControlFlow::Continue;
use signal_hook::consts::signal::*;
use signal_hook::flag;
use std::fs;

#[derive(Debug)]
struct Notification {
    summary: String,
    body: String,
    icon_id: Option<String>,
    urgency: String,
}

fn read_notifications() -> Vec<Notification> {
    let content = fs::read_to_string("/tmp/notiv.dat").unwrap_or_default();
    let entries: Vec<&str> = content.split("}\n{").collect();

    entries
        .into_iter()
        .map(|raw| {
            let clean = raw.replace("{", "").replace("}", "");
            let mut summary = String::new();
            let mut body = String::new();
            let mut icon_id = None;
            let mut urgency = "NORMAL".to_string();

            for line in clean.lines() {
                if line.trim_start().starts_with("summary:") {
                    summary = line.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('\'').to_string();
                } else if line.trim_start().starts_with("body:") {
                    body = line.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('\'').to_string();
                } else if line.trim_start().starts_with("icon_id:") {
                    let val = line.splitn(2, ':').nth(1).unwrap_or("").trim().trim_matches('\'');
                    if !val.is_empty() && val != "(null)" {
                        icon_id = Some(val.to_string());
                    }
                } else if line.trim_start().starts_with("urgency:") {
                    urgency = line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string();
                }
            }

            Notification { summary, body, icon_id, urgency }
        })
        .collect()
}

pub fn build_window(app: &Application) {
    let css = CssProvider::new();
    let home_dir = env::var("HOME").unwrap();
    let css_path = format!("{}/.config/capsule/style.css", home_dir);
    let file = File::for_path(css_path);

    css.load_from_file(&file);

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let reload_flag = Arc::new(AtomicBool::new(false));
    flag::register(SIGUSR1, Arc::clone(&reload_flag)).unwrap();

    // Periodic check loop (you can also use timeout_add)
    gtk4::glib::timeout_add_seconds_local(1, move || {
        if reload_flag.swap(false, Ordering::Relaxed) {
            eprintln!("Reloading CSS...");
            css.load_from_file(&file);
        }
        Continue
    });

    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_default_size(500, 600);
    window.set_namespace(Some("notification_bubble"));

    let main_box = GtkBox::new(Orientation::Vertical, 12);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    let mut notifications = read_notifications();
    notifications.reverse();

    for note in notifications {
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        
        if let Some(id) = &note.icon_id {
            let path = format!("/tmp/icons/{}.png", id); // You can adapt this
            if std::path::Path::new(&path).exists() {
                let image = Image::from_file(path);
                image.set_pixel_size(32);
                hbox.append(&image);
            }
        }

        let text = format!("<b>{}</b>\n{}", note.summary, note.body);
        let label = Label::new(Some(&text));
        label.set_use_markup(true);
        label.set_wrap(true);
        label.set_xalign(0.0);

        // Apply urgency CSS class
        let urgency_class = match note.urgency.to_lowercase().as_str() {
            "low" => "noti-low",
            "critical" => "noti-critical",
            _ => "noti-normal",
        };
        label.add_css_class(urgency_class);
        hbox.append(&label);

        main_box.append(&hbox);
    }

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&main_box));
    window.set_child(Some(&scroll));
    window.show();
}