use zbus::connection::Builder;
use tokio::sync::mpsc;
use gtk4::{ApplicationWindow, Box as GtkBox, Button, Image, Label, glib, prelude::*};
use gtk4::glib::clone;
use tokio::sync::mpsc::UnboundedReceiver;
use std::cell::{RefCell, Cell};
use std::rc::Rc;
use std::collections::VecDeque;
use rodio::{Decoder, OutputStream, Sink};
use std::fs::File;
use std::thread;
use std::io::{self, BufRead, BufReader};
use std::time::Duration;
use gtk4_layer_shell::LayerShell;

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub _timestamp: std::time::Instant,
    pub _actions: Vec<String>
}

struct NotificationServer {
    sender: mpsc::UnboundedSender<Notification>,
    next_id: std::sync::atomic::AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<String>,
        hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let icon = if !app_icon.is_empty() {
            app_icon.to_string()
        } else if let Some(val) = hints.get("image-path") {
            val.to_string().trim_matches('"').to_string()
        } else if let Some(val) = hints.get("app-icon") {
            val.to_string().trim_matches('"').to_string()
        } else {
            app_name.to_lowercase()
        };

        let notif = Notification {
            id,
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            icon,
            _timestamp: std::time::Instant::now(),
            _actions,
        };

        let _ = self.sender.send(notif);
        id
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "persistence".into()]
    }

    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("capsule", "ekah", "1.0", "1.2")
    }

    async fn close_notification(&self, _id: u32) {}
}

pub fn spawn_messaging_daemon() -> UnboundedReceiver<Notification> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async move {
                let server = NotificationServer {
                    sender: tx,
                    next_id: std::sync::atomic::AtomicU32::new(1),
                };

                let _conn = Builder::session()
                    .expect("dbus session")
                    .name("org.freedesktop.Notifications")
                    .expect("dbus name")
                    .serve_at("/org/freedesktop/Notifications", server)
                    .expect("serve_at")
                    .build()
                    .await
                    .expect("dbus connection");

                std::future::pending::<()>().await;
            });
    });

    rx
}

fn play_notification_sound() {
    let file = match File::open("/var/lib/cynager/info.probe") {
        Ok(f) => f,
        Err(_) => return,
    };
    let reader = io::BufReader::new(file);
    let mut in_set_block = false;
    let mut dnd = String::new();

    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim().to_string();
        if trimmed == ":set" {
            in_set_block = true;
            continue;
        }
        if trimmed == ":end" {
            in_set_block = false;
            continue;
        }
        if in_set_block && trimmed.starts_with("dnd") {
            let parts: Vec<&str> = trimmed.split(':').collect();
            if parts.len() >= 2 {
                dnd = parts[1].trim().to_string();
            }
        }
    }

    if dnd == "false" {
        thread::spawn(|| {
            let (_stream, stream_handle) = match OutputStream::try_default() {
                Ok(v) => v,
                Err(_) => return,
            };
            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Ok(file) = File::open("/var/lib/cynager/niri/sound/notiv/notiv.mp3") {
                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
        });
    }
}

pub fn connect_notifications_to_dock(
    mut rx: UnboundedReceiver<Notification>,
    noti_window: &GtkBox,
    main_window: &ApplicationWindow,
    app_img: &Image,
    cos_btn: &Button,
    badge: &Label,
    noti_all: &GtkBox,
) { 
    
    let history: Rc<RefCell<VecDeque<Notification>>> =
        Rc::new(RefCell::new(VecDeque::with_capacity(50)));

    let pending_count: Rc<Cell<u32>>  = Rc::new(Cell::new(0));
    let is_expanded:   Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let current_width: Rc<Cell<f64>>  = Rc::new(Cell::new(300.0));
    let ctx = gtk4::glib::MainContext::default();
    ctx.spawn_local(clone!(
        #[strong] noti_window,
        #[strong] main_window,
        #[strong] app_img,
        #[strong] cos_btn,
        #[strong] badge,
        #[strong] noti_all,
        async move {
            while let Some(notif) = rx.recv().await {
                {
                    let mut h = history.borrow_mut();
                    if h.len() == 50 { h.pop_front(); }
                    h.push_back(notif.clone());
                }

                let notification_icon = Image::from_file("/var/lib/cynager/icons/noti.svg");
                notification_icon.set_icon_size(gtk4::IconSize::Normal);
                notification_icon.set_css_classes(&["notiIcon"]);
                notification_icon.set_height_request(28);

                if std::path::Path::new(&notif.icon).is_absolute()
                    && std::path::Path::new(&notif.icon).exists()
                {
                    app_img.set_from_file(Some(&notif.icon));
                } else {
                    app_img.set_from_file(Some("/var/lib/cynager/icons/noti.svg"));
                }
                cos_btn.set_css_classes(&["spinning-coin", "cosIcon"]);
                badge.set_visible(true);
                badge.set_text(&format!("{}\n{}", notif.summary, notif.body));

                play_notification_sound();

                let noti_label_sum = Label::new(Some(&notif.summary));
                noti_label_sum.set_css_classes(&["notificationAllLabelSummary"]);
                noti_label_sum.set_halign(gtk4::Align::Start);

                let noti_label_bod = Label::new(Some(&notif.body));
                noti_label_bod.set_css_classes(&["notificationAllLabelBody"]);
                noti_label_bod.set_halign(gtk4::Align::Start);
                noti_label_bod.set_wrap(true);
                noti_label_bod.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                noti_label_bod.set_width_request(50);
                noti_label_bod.set_max_width_chars(10);
                noti_label_bod.set_lines(2);
                noti_label_bod.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let noti_label_all = GtkBox::new(gtk4::Orientation::Horizontal, 20);
                noti_label_all.append(&noti_label_sum);
                noti_label_all.append(&noti_label_bod);

                let noti_all_box = GtkBox::new(gtk4::Orientation::Horizontal, 5);
                noti_all_box.set_css_classes(&["notificationAll"]);
                noti_all_box.set_width_request(500);
                noti_all_box.set_height_request(30);
                noti_all_box.set_hexpand(true);
                noti_all_box.set_margin_start(10);
                noti_all_box.set_margin_end(10);
                noti_all_box.set_halign(gtk4::Align::Center);

                let delete_btn = Button::new();
                let delete = Image::from_file("/var/lib/cynager/icons/close.svg");
                delete.set_icon_size(gtk4::IconSize::Normal);
                delete_btn.set_child(Some(&delete));
                delete_btn.set_css_classes(&["deleteBtn"]);
                delete_btn.set_width_request(28);
                delete_btn.set_height_request(28);
                delete_btn.set_hexpand(true);
                delete_btn.set_halign(gtk4::Align::End);
                delete_btn.set_cursor_from_name(Some("pointer"));

                noti_all_box.append(&notification_icon);
                noti_all_box.append(&noti_label_all);
                noti_all_box.append(&Label::builder()
                    .label(&notif.app_name)
                    .css_classes(["appName"])
                    .hexpand(true)
                    .halign(gtk4::Align::End)
                    .build()
                );
                noti_all_box.append(&delete_btn);


                let clear_all_btn = Button::new();
                let clear = Image::from_file("/var/lib/cynager/icons/delete.svg");

                clear.set_icon_size(gtk4::IconSize::Large);
                clear_all_btn.set_child(Some(&clear));
                clear_all_btn.set_css_classes(&["clearBtn"]);
                clear_all_btn.set_width_request(30);
                clear_all_btn.set_height_request(30);
                clear_all_btn.set_hexpand(true);
                clear_all_btn.set_halign(gtk4::Align::Fill);
                clear_all_btn.set_vexpand(true);
                clear_all_btn.set_valign(gtk4::Align::Center);
                clear_all_btn.set_cursor_from_name(Some("pointer"));

                let noti_all_clone = noti_all.clone();
                clear_all_btn.connect_clicked( move |_| {
                    noti_all_clone.add_css_class("vanish");
                    let noti_all_clone = noti_all_clone.clone();
                    glib::timeout_add_local(Duration::from_secs(1), move || {
                        while let Some(child) = noti_all_clone.first_child() {
                            noti_all_clone.remove(&child);
                        }
                        if let Some(root) = noti_all_clone.root() {
                            if let Some(window) = root.downcast_ref::<gtk4::Window>() {
                                window.set_visible(false);
                                window.set_visible(true);
                            }
                        }
                        noti_all_clone.remove_css_class("vanish");
                        noti_all_clone.set_height_request(10);
                        glib::ControlFlow::Break
                    });                 
                });

                if noti_all.first_child().is_none() {
                    noti_all.append(&clear_all_btn);
                }

                noti_all.prepend(&noti_all_box);

                let noti_all_clone = noti_all.clone();
                delete_btn.connect_clicked( move |_| {
                    noti_all_clone.remove(&noti_all_box);

                    let first = noti_all_clone.first_child();
                    let last  = noti_all_clone.last_child();
                    let is_only_clear_btn = first.is_some() && first == last;
 
                    if is_only_clear_btn {
                        noti_all_clone.add_css_class("vanish");
                        let noti_all_clone = noti_all_clone.clone();
                        glib::timeout_add_local(Duration::from_secs(1), move || {
                            if let Some(child) = noti_all_clone.first_child() {
                                noti_all_clone.remove(&child);
                            }
                            if let Some(root) = noti_all_clone.root() {
                                if let Some(window) = root.downcast_ref::<gtk4::Window>() {
                                    window.set_visible(false);
                                    window.set_visible(true);
                                }
                            }
                            noti_all_clone.remove_css_class("vanish");
                            glib::ControlFlow::Break
                        });
                    }
                });



                pending_count.set(pending_count.get() + 1);

                let display  = gtk4::gdk::Display::default().expect("no display");
                let monitors = display.monitors();

                if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
                    let geometry     = monitor.geometry();
                    let target_width = (geometry.width() as f64 * 0.8) as i32;
                    let start_width  = 300i32;
                    let increment_per_frame =
                        (target_width - start_width) as f64 / (1500.0 / (1000.0 / 114.0));

                    if !is_expanded.get() {
                        is_expanded.set(true);
                        current_width.set(start_width as f64);

                        // main_window.set_width_request(target_width + 50);
                        noti_window.set_width_request(start_width);
                        noti_window.set_css_classes(&["timeCapsule"]);
                        main_window.set_layer(gtk4_layer_shell::Layer::Overlay);

                        let noti_window_anim   = noti_window.clone();
                        let current_width_anim = Rc::clone(&current_width);
                        let main_c = main_window.clone();

                        gtk4::glib::timeout_add_local(
                            std::time::Duration::from_millis(6),
                            move || {
                                let next_w = current_width_anim.get() + increment_per_frame;
                                if next_w >= target_width as f64 {
                                    current_width_anim.set(target_width as f64);
                                    noti_window_anim.set_width_request(target_width);
                                    main_c.set_width_request(current_width_anim.get() as i32 + 30);
                                    noti_window_anim.set_css_classes(&["blip", "timeCapsule"]);
                                    return gtk4::glib::ControlFlow::Break;
                                }
                                current_width_anim.set(next_w);
                                noti_window_anim.set_width_request(next_w as i32);
                                gtk4::glib::ControlFlow::Continue
                            },
                        );
                    } else {
                        noti_window.remove_css_class("blip");
                        let noti_window_blip = noti_window.clone();
                        let main_c = main_window.clone();
                        let current_width_anim = Rc::clone(&current_width);
                        gtk4::glib::timeout_add_local(
                            std::time::Duration::from_millis(6),
                            move || {
                                noti_window_blip.add_css_class("blip");
                                main_c.set_width_request(current_width_anim.get() as i32 + 30);
                                gtk4::glib::ControlFlow::Break
                            },
                        );
                    }

                    let pending_count_hide = Rc::clone(&pending_count);
                    let is_expanded_hide   = Rc::clone(&is_expanded);
                    let current_width_hide = Rc::clone(&current_width);
                    let noti_window_hide   = noti_window.clone();
                    let main_window_hide   = main_window.clone();
                    let app_img_hide       = app_img.clone();
                    let cos_btn_hide       = cos_btn.clone();
                    let badge_hide         = badge.clone();

                    glib::timeout_add_local(std::time::Duration::from_millis(10000), move || {
                        let remaining = pending_count_hide.get().saturating_sub(1);
                        pending_count_hide.set(remaining);

                        if remaining == 0 {
                            badge_hide.set_text("");
                            app_img_hide.set_from_file(Some("/var/lib/cynager/icons/cos.svg"));
                            cos_btn_hide.remove_css_class("spinning-coin");
                            is_expanded_hide.set(false);

                            let noti_window_c   = noti_window_hide.clone();
                            let current_width_c = Rc::clone(&current_width_hide);
                            let main_c          = main_window_hide.clone();

                            glib::timeout_add_local(
                                std::time::Duration::from_millis(6),
                                move || {
                                    let next_w = current_width_c.get() - increment_per_frame;
                                    if next_w <= start_width as f64 {
                                        noti_window_c.set_width_request(start_width);
                                        noti_window_c.remove_css_class("blip");
                                        main_c.set_layer(gtk4_layer_shell::Layer::Top);
                                        return glib::ControlFlow::Break;
                                    }
                                    current_width_c.set(next_w);
                                    noti_window_c.set_width_request(next_w as i32);
                                    glib::ControlFlow::Continue
                                },
                            );
                        }
                        glib::ControlFlow::Break
                    });
                }
            }
        }
    ));
}