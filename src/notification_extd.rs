use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, Button, EventControllerMotion
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::env;
use gtk4::gio::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use glib::ControlFlow::Continue;
use signal_hook::consts::signal::*;
use signal_hook::flag;

#[derive(Debug)]
struct Notification {
    summary: String,
    body: String,
    urgency: String,
}

fn read_notifications() -> Vec<Notification> {
    let content = std::fs::read_to_string("/tmp/notiv.dat").unwrap_or_default();
    let entries: Vec<&str> = content.split("}\n{").collect();

    entries
        .into_iter()
        .map(|raw| {
            let clean = raw.replace("{", "").replace("}", "");
            let mut summary = String::new();
            let mut body_lines = Vec::new();
            let mut urgency = "NORMAL".to_string();

            let mut in_body = false;

            for line in clean.lines() {
                let trimmed = line.trim_start();

                if trimmed.starts_with("summary:") {
                    summary = trimmed
                        .splitn(2, ':')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .trim_matches('\'')
                        .to_string();
                    in_body = false;
                } else if trimmed.starts_with("body:") {
                    let first_line = trimmed
                        .splitn(2, ':')
                        .nth(1)
                        .unwrap_or("")
                        .trim_start()
                        .to_string();
                    body_lines.push(first_line);
                    in_body = true;
                } else if trimmed.starts_with("urgency:") {
                    urgency = trimmed
                        .splitn(2, ':')
                        .nth(1)
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    in_body = false;
                } else if trimmed.starts_with("icon:") {
                    break; // Stop parsing after `icon:`
                } else if in_body {
                    body_lines.push(trimmed.to_string());
                }
            }

            // Now clean up the first and last lines
            if let Some(first) = body_lines.first_mut() {
                if first.starts_with('\'') {
                    *first = first[1..].to_string();
                }
            }

            if let Some(last) = body_lines.last_mut() {
                if last.ends_with('\'') {
                    let len = last.len();
                    *last = last[..len - 1].to_string();
                }
            }

            let body = body_lines.join("\n");

            Notification {
                summary,
                body,
                urgency,
            }
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

    gtk4::glib::timeout_add_seconds_local(1, move || {
        if reload_flag.swap(false, Ordering::Relaxed) {
            css.load_from_file(&file);
        }
        Continue
    });

    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.set_namespace(Some("capsule_notifications"));

    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Left, true);

    let main_box = GtkBox::new(Orientation::Vertical, 7);
    main_box.set_margin_top(20);
    main_box.set_margin_bottom(20);
    main_box.set_margin_start(20);
    main_box.set_margin_end(20);

    let mut notifications = read_notifications();
    notifications.reverse();

    for note in notifications {
        let hbox = GtkBox::new(Orientation::Horizontal, 8);

        if note.summary.is_empty() && note.body.is_empty() {
            let label = Label::new(Some(""));
            label.set_markup("no <i>new</i> notifications");
            label.set_vexpand(true);
            label.set_hexpand(true);
            label.set_valign(gtk4::Align::Center);
            label.set_halign(gtk4::Align::Fill);
            label.set_justify(gtk4::Justification::Center);
            label.set_widget_name("notification_heading");
            label.set_opacity(0.5);
            hbox.append(&label);
        } else {
            let text = format!("<b>{}</b>\n{}", note.summary, note.body);
            let label = Label::new(Some(&text));
            label.set_use_markup(true);
            label.set_hexpand(true);
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
        }

        main_box.append(&hbox);
    }

    let scroll = gtk4::ScrolledWindow::new();
    scroll.set_child(Some(&main_box));
    scroll.set_widget_name("notification_scroller");
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);

    let noti_bubble = GtkBox::new(Orientation::Vertical, 5);
    noti_bubble.set_widget_name("notification_bubble");
    noti_bubble.set_vexpand(true);
    noti_bubble.set_hexpand(true);
    noti_bubble.set_valign(gtk4::Align::Center);
    noti_bubble.set_halign(gtk4::Align::Center);
    noti_bubble.set_size_request(600, 600);
    noti_bubble.set_margin_top(90);

    
    let exit_button = Button::builder().child(&Label::new(Some("exit"))).build();
    exit_button.set_widget_name("close_button");
    exit_button.set_hexpand(true);
    exit_button.set_margin_bottom(20);
    exit_button.set_halign(gtk4::Align::Center);
    exit_button.set_valign(gtk4::Align::Start);


    let noti_shadow = ApplicationWindow::new(app);

    let window_clone = window.clone();
    let noti_shadow_clone = noti_shadow.clone();
    exit_button.connect_clicked(move |_| {
        window_clone.close();
        noti_shadow_clone.close();
    }); 

    let noti_label = Label::new(Some("Notifications"));
    noti_label.set_widget_name("notification_heading");
    noti_label.set_hexpand(true);
    noti_label.set_halign(gtk4::Align::Start);
    noti_label.set_margin_start(10);
    noti_label.set_margin_bottom(10);
    

    let noti_label_button = Button::builder().child(&noti_label).css_classes(["notification_heading_button"]).build();
    noti_label_button.set_hexpand(false);
    noti_label_button.set_halign(gtk4::Align::Start);
    let controller = EventControllerMotion::new();
    let con_clone = controller.clone();
    noti_label_button.add_controller(controller);

    let label_enter = noti_label.clone();
    con_clone.connect_enter(move |_ctrl, _x, _y| {
        label_enter.set_text("Clear Notifications");
    });

    let label_leave = noti_label.clone();
    con_clone.connect_leave(move |_ctrl| {
        label_leave.set_text("Notifications");
    });

    let window_clone2 = window.clone();
    let noti_shadow_clone2 = noti_shadow.clone();
    noti_label_button.connect_clicked(move |_| {
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("truncate -s 0 /tmp/notiv.dat")
            .output();
        window_clone2.close();
        noti_shadow_clone2.close();
    });

    noti_bubble.append(&exit_button);
    noti_bubble.append(&noti_label_button);
    noti_bubble.append(&scroll);

    window.set_child(Some(&noti_bubble));


    noti_shadow.init_layer_shell();
    noti_shadow.set_layer(Layer::Top);
    noti_shadow.set_namespace(Some("capsule_notifications_shadow"));

    noti_shadow.set_anchor(Edge::Top, true);
    noti_shadow.set_anchor(Edge::Right, true);
    noti_shadow.set_anchor(Edge::Left, true);

    let shadow = GtkBox::new(Orientation::Vertical, 0);
    shadow.append(&Label::new(Some("this is supposed to be transperent")));
    shadow.set_widget_name("shadow");
    shadow.set_vexpand(true);
    shadow.set_hexpand(true);
    shadow.set_valign(gtk4::Align::Center);
    shadow.set_halign(gtk4::Align::Center);
    shadow.set_size_request(600, 600);
    shadow.set_margin_top(90);
    shadow.set_margin_bottom(100);

    noti_shadow.set_child(Some(&shadow));
    noti_shadow.show();

    window.show();
}
