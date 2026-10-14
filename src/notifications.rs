use zbus::connection::Builder;
use tokio::sync::mpsc;
use gtk4::{ApplicationWindow, Box as GtkBox, Button, Image, Label, Revealer, ScrolledWindow, glib, prelude::*};
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
    pub _id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub _timestamp: std::time::Instant,
    pub _actions: Vec<String>
}

struct LastNotiGroup {
    summary: String,
    details_vbox: GtkBox,
    scroller: ScrolledWindow,
    body_label: Label,
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
        let _id = self
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
            _id,
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            icon,
            _timestamp: std::time::Instant::now(),
            _actions,
        };

        let _ = self.sender.send(notif);
        _id
    }

    async fn get_capabilities(&self) -> Vec<String> {
        return vec!["body".into(), "persistence".into()]
    }

    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("capsule", "ekah", "1.0", "1.2")
    }

    async fn close_notification(&self, _id: u32) {}
}

fn clean_body(raw: &str) -> String {
    let mut s = raw.replace("\r\n", "\n").replace('\r', "\n");

    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut tag = String::new();
    for c in s.chars() {
        if in_tag {
            if c == '>' {
                let t = tag.trim().trim_start_matches('/').trim_end_matches('/').trim().to_lowercase();
                let name = t.split_whitespace().next().unwrap_or("");
                if matches!(name, "br" | "p" | "div" | "li" | "tr") {
                    out.push('\n');
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(c);
            }
        } else if c == '<' {
            in_tag = true;
        } else {
            out.push(c);
        }
    }
    if in_tag {
        out.push('<');
        out.push_str(&tag);
    }
    s = out;

    s = s
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&");

    let mut lines: Vec<String> = Vec::new();
    let mut last_blank = true;
    for line in s.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.is_empty() {
            if !last_blank {
                lines.push(String::new());
            }
            last_blank = true;
        } else {
            lines.push(collapsed);
            last_blank = false;
        }
    }
    while lines.last().map(|l| l.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

const SCROLLER_MAX_HEIGHT: i32 = 520;
const SCROLLER_MIN_HEIGHT: i32 = 30;
const LINE_HEIGHT_PX: i32 = 22;
const WRAP_CHARS: usize = 45;

fn wrap_in_scroller(content: &GtkBox) -> ScrolledWindow {
    let scroller = ScrolledWindow::new();
    scroller.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroller.set_propagate_natural_height(true);
    scroller.set_max_content_height(SCROLLER_MAX_HEIGHT);
    scroller.set_hexpand(true);
    scroller.set_vexpand(false);
    scroller.set_valign(gtk4::Align::Start);
    scroller.set_css_classes(&["notiScroller"]);
    scroller.set_child(Some(content));
    fit_scroller(&scroller, content);
    scroller
}

fn fit_scroller(scroller: &ScrolledWindow, content: &GtkBox) {
    let mut lines = 0i32;
    let mut labels = 0i32;
    let mut child = content.first_child();
    while let Some(w) = child {
        if let Some(label) = w.downcast_ref::<Label>() {
            labels += 1;
            for line in label.text().split('\n') {
                let n = line.chars().count();
                lines += ((n + WRAP_CHARS - 1) / WRAP_CHARS).max(1) as i32;
            }
        }
        child = w.next_sibling();
    }
    let needed = lines * LINE_HEIGHT_PX + (labels - 1).max(0) * 6 + 15;
    let height = needed.clamp(SCROLLER_MIN_HEIGHT, SCROLLER_MAX_HEIGHT);
    scroller.set_min_content_height(height);
}

fn is_it_dnd() -> String {
    let file = match File::open("/var/lib/cynager/info.probe") {
        Ok(f) => f,
        Err(_) => return "false".to_string(),
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

    dnd
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
    if is_it_dnd() == "false" {
        thread::spawn(|| {
            let (_stream, stream_handle) = match OutputStream::try_default() {
                Ok(v) => v,
                Err(_) => return,
            };
            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Ok(file) = File::open("/var/lib/cynager/sound/notiv/notiv.mp3") {
                if let Ok(source) = Decoder::new(BufReader::new(file)) {
                    sink.append(source);
                    sink.sleep_until_end();
                }
            }
        });
    }
}

fn clear_all_notifications(
    noti_all: &GtkBox,
    last_group: &Rc<RefCell<Option<LastNotiGroup>>>,
) {
    *last_group.borrow_mut() = None;
    noti_all.add_css_class("vanish");
    let noti_all = noti_all.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        while let Some(child) = noti_all.first_child() {
            noti_all.remove(&child);
        }
        if let Some(root) = noti_all.root() {
            if let Some(window) = root.downcast_ref::<gtk4::Window>() {
                window.set_visible(false);
                window.set_visible(true);
            }
        }
        noti_all.remove_css_class("vanish");
        noti_all.set_height_request(10);
        glib::ControlFlow::Break
    });
}

fn build_clear_button(
    noti_all: &GtkBox,
    last_group: &Rc<RefCell<Option<LastNotiGroup>>>,
) -> Button {
    let clear_all_btn = Button::new();
    let clear = Image::from_file("/var/lib/cynager/icons/delete.svg");

    clear.set_icon_size(gtk4::IconSize::Large);
    clear_all_btn.set_child(Some(&clear));
    clear_all_btn.set_css_classes(&["clearBtn"]);
    clear_all_btn.set_width_request(30);
    clear_all_btn.set_height_request(30);
    clear_all_btn.set_margin_bottom(5);
    clear_all_btn.set_hexpand(true);
    clear_all_btn.set_halign(gtk4::Align::Fill);
    clear_all_btn.set_vexpand(true);
    clear_all_btn.set_valign(gtk4::Align::End);
    clear_all_btn.set_cursor_from_name(Some("pointer"));

    let noti_all = noti_all.clone();
    let last_group = Rc::clone(last_group);
    clear_all_btn.connect_clicked(move |_| clear_all_notifications(&noti_all, &last_group));
    clear_all_btn
}

pub fn connect_notifications_to_dock(
    mut rx: UnboundedReceiver<Notification>,
    noti_capsule: &GtkBox,
    main_window: &ApplicationWindow,
    noti_all_window: &ApplicationWindow,
    app_img: &Image,
    cos_btn: &Button,
    badge: &Label,
    badge_head: &Label,
    noti_all: &GtkBox,
) { 
    
    let history: Rc<RefCell<VecDeque<Notification>>> =
        Rc::new(RefCell::new(VecDeque::with_capacity(50)));

    let pending_count: Rc<Cell<u32>>  = Rc::new(Cell::new(0));
    let is_expanded:   Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let current_width: Rc<Cell<f64>>  = Rc::new(Cell::new(300.0));
    let last_group: Rc<RefCell<Option<LastNotiGroup>>> = Rc::new(RefCell::new(None));
    let ctx = gtk4::glib::MainContext::default();
    ctx.spawn_local(clone!(
        #[strong] noti_capsule,
        #[strong] main_window,
        #[strong] noti_all_window,
        #[strong] app_img,
        #[strong] cos_btn,
        #[strong] badge,
        #[strong] badge_head,
        #[strong] noti_all,
        async move {
            while let Some(notif) = rx.recv().await {
                {
                    let mut h = history.borrow_mut();
                    if h.len() == 50 { h.pop_front(); }
                    h.push_back(notif.clone());
                }

                let notification_icon = Image::from_file("/var/lib/cynager/icons/noti.svg");
                notification_icon.set_css_classes(&["notiIcon"]);
                notification_icon.set_pixel_size(30);

                let display = gtk4::gdk::Display::default().expect("[notifications] Failed to get default GDK display");
                let icon_theme = gtk4::IconTheme::for_display(&display);

                if icon_theme.has_icon(&notif.icon) {
                    app_img.set_icon_name(Some(&notif.icon));
                    notification_icon.set_icon_name(Some(&notif.icon));
                } else {
                    app_img.set_from_file(Some("/var/lib/cynager/icons/noti.svg"));
                }

                cos_btn.set_css_classes(&["spinning-coin", "cosIcon"]);
                badge.set_visible(true);
                badge_head.set_visible(true);
                badge.set_ellipsize(gtk4::pango::EllipsizeMode::End);
                badge_head.set_text(&format!("{}",notif.summary));
                badge.set_text(&format!("{}", notif.body));

                play_notification_sound();

                let is_duplicate = last_group
                    .borrow()
                    .as_ref()
                    .map(|g| g.summary == notif.summary)
                    .unwrap_or(false);

                let clean = clean_body(&notif.body);

                if is_duplicate {
                    if let Some(group) = last_group.borrow().as_ref() {
                        group.body_label.set_text(&clean.replace('\n', " "));

                        let grouped_label = Label::new(Some(&clean));
                        grouped_label.set_css_classes(&["notificationAllLabelBody"]);
                        grouped_label.set_wrap(true);
                        grouped_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                        grouped_label.set_max_width_chars(45);
                        grouped_label.set_halign(gtk4::Align::Start);

                        group.details_vbox.append(&grouped_label);
                        fit_scroller(&group.scroller, &group.details_vbox);
                    }
                } else {

                let noti_label_sum = Label::new(Some(&notif.summary));
                noti_label_sum.set_css_classes(&["notificationAllLabelSummary"]);
                noti_label_sum.set_halign(gtk4::Align::Start);

                let noti_label_bod = Label::new(Some(&clean.replace('\n', " ")));
                noti_label_bod.set_css_classes(&["notificationAllLabelBody"]);
                noti_label_bod.set_halign(gtk4::Align::Start);
                noti_label_bod.set_valign(gtk4::Align::Center);
                noti_label_bod.set_wrap(false);
                noti_label_bod.set_single_line_mode(true);
                noti_label_bod.set_width_request(100);
                noti_label_bod.set_ellipsize(gtk4::pango::EllipsizeMode::End);

                let noti_label_all = GtkBox::new(gtk4::Orientation::Vertical, 2);
                noti_label_all.set_margin_start(10);
                noti_label_all.append(&Label::builder()
                    .label(if notif.app_name.trim().is_empty() {
                        "Notification"
                    } else {
                        &notif.app_name
                    })
                    .css_classes(["appName"])
                    .hexpand(true)
                    .halign(gtk4::Align::Start)
                    .build()
                );
                noti_label_all.append(&noti_label_sum);

                let noti_all_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
                noti_all_box.set_css_classes(&["notificationAll"]);
                noti_all_box.set_width_request(500);
                noti_all_box.set_hexpand(true);
                noti_all_box.set_margin_start(10);
                noti_all_box.set_margin_end(10);
                noti_all_box.set_vexpand(false);
                noti_all_box.set_valign(gtk4::Align::End);
                noti_all_box.set_halign(gtk4::Align::Center);

                let noti_row = GtkBox::new(gtk4::Orientation::Horizontal, 0);
                noti_row.set_height_request(30);
                noti_row.set_hexpand(true);

                let pop_label = Label::new(Some(&clean));
                pop_label.set_css_classes(&["notificationAllLabelBody"]);
                pop_label.set_wrap(true);
                pop_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
                pop_label.set_max_width_chars(45);
                pop_label.set_halign(gtk4::Align::Start);

                let details_vbox = GtkBox::new(gtk4::Orientation::Vertical, 6);
                details_vbox.set_css_classes(&["notiPopupBox"]);
                details_vbox.set_margin_top(10);
                details_vbox.set_margin_bottom(5);
                details_vbox.append(&pop_label);

                let revealer = Revealer::new();
                revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
                revealer.set_transition_duration(250);
                revealer.set_reveal_child(false);
                let scroller = wrap_in_scroller(&details_vbox);
                revealer.set_child(Some(&scroller));

                revealer.connect_reveal_child_notify(clone!(
                    #[weak] noti_all_window,
                    move |_| {
                        noti_all_window.set_layer(gtk4_layer_shell::Layer::Overlay);
                    }
                ));

                revealer.connect_child_revealed_notify(clone!(
                    #[weak] noti_all_window,
                    move |r| {
                        if !r.is_child_revealed() {
                            let (w, _) = noti_all_window.default_size();
                            noti_all_window.set_default_size(w, -1);
                            noti_all_window.set_layer(gtk4_layer_shell::Layer::Bottom);
                        }
                    }
                ));

                let hover_ctrl = gtk4::EventControllerMotion::new();
                let hover_pending: Rc<Cell<bool>> = Rc::new(Cell::new(false));

                let pending_enter = Rc::clone(&hover_pending);
                let revealer_enter = revealer.clone();
                hover_ctrl.connect_enter(move |_, _, _| {
                    pending_enter.set(true);
                    let pending_timeout = Rc::clone(&pending_enter);
                    let revealer_timeout = revealer_enter.clone();
                    glib::timeout_add_local(Duration::from_millis(500), move || {
                        if pending_timeout.get() {
                            revealer_timeout.set_reveal_child(true);
                        }
                        glib::ControlFlow::Break
                    });
                });

                let pending_leave = Rc::clone(&hover_pending);
                let revealer_leave = revealer.clone();
                hover_ctrl.connect_leave(move |_| {
                    pending_leave.set(false);
                    revealer_leave.set_reveal_child(false);
                });

                noti_all_box.add_controller(hover_ctrl);

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

                noti_row.append(&notification_icon);
                noti_row.append(&noti_label_all);
                noti_row.append(&delete_btn);

                noti_all_box.append(&noti_row);
                noti_all_box.append(&revealer);

                if noti_all.first_child().is_none() {
                    noti_all.append(&build_clear_button(&noti_all, &last_group));
                }

                let header = noti_all.first_child();
                noti_all.insert_child_after(&noti_all_box, header.as_ref());

                let noti_all_clone = noti_all.clone();
                let last_group_del = Rc::clone(&last_group);
                let details_vbox_del = details_vbox.clone();
                delete_btn.connect_clicked( move |_| {
                    noti_all_clone.remove(&noti_all_box);

                    {
                        let mut lg = last_group_del.borrow_mut();
                        let is_current_group = lg
                            .as_ref()
                            .map(|g| g.details_vbox == details_vbox_del)
                            .unwrap_or(false);
                        if is_current_group {
                            *lg = None;
                        }
                    }

                    let mut remaining = 0;
                    let mut child = noti_all_clone.first_child();
                    while let Some(w) = child {
                        remaining += 1;
                        child = w.next_sibling();
                    }
                    if remaining <= 2 {
                        clear_all_notifications(&noti_all_clone, &last_group_del);
                    }
                });

                *last_group.borrow_mut() = Some(LastNotiGroup {
                    summary: notif.summary.clone(),
                    details_vbox,
                    scroller,
                    body_label: noti_label_bod,
                });

                }

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

                        main_window.set_default_size(target_width + 50, -1);
                        noti_capsule.set_width_request(start_width);
                        noti_capsule.set_css_classes(&["timeCapsule", "notiCapsule"]);
                        if is_it_dnd() == "false" {
                            main_window.set_layer(gtk4_layer_shell::Layer::Overlay);
                        }

                        main_window.set_namespace(Some("notification!"));
                        let noti_capsule_anim   = noti_capsule.clone();
                        let current_width_anim = Rc::clone(&current_width);
                        let main_c = main_window.clone();

                        gtk4::glib::timeout_add_local(
                            std::time::Duration::from_millis(6),
                            move || {
                                let next_w = current_width_anim.get() + increment_per_frame;
                                if next_w >= target_width as f64 {
                                    current_width_anim.set(target_width as f64);
                                    noti_capsule_anim.set_width_request(target_width);
                                    main_c.set_width_request(current_width_anim.get() as i32 + 30);
                                    noti_capsule_anim.set_css_classes(&["blip", "timeCapsule", "notiCapsule"]);
                                    return gtk4::glib::ControlFlow::Break;
                                }
                                current_width_anim.set(next_w);
                                noti_capsule_anim.set_width_request(next_w as i32);
                                gtk4::glib::ControlFlow::Continue
                            },
                        );
                    } else {
                        noti_capsule.remove_css_class("blip");
                        let noti_capsule_blip = noti_capsule.clone();
                        let main_c = main_window.clone();
                        let current_width_anim = Rc::clone(&current_width);
                        gtk4::glib::timeout_add_local(
                            std::time::Duration::from_millis(6),
                            move || {
                                noti_capsule_blip.add_css_class("blip");
                                main_c.set_width_request(current_width_anim.get() as i32 + 30);
                                gtk4::glib::ControlFlow::Break
                            },
                        );
                    }

                    let pending_count_hide = Rc::clone(&pending_count);
                    let is_expanded_hide   = Rc::clone(&is_expanded);
                    let current_width_hide = Rc::clone(&current_width);
                    let noti_capsule_hide   = noti_capsule.clone();
                    let main_window_hide   = main_window.clone();
                    let app_img_hide       = app_img.clone();
                    let cos_btn_hide       = cos_btn.clone();
                    let badge_hide         = badge.clone();
                    let badgeh_hide         = badge_head.clone();

                    glib::timeout_add_local(std::time::Duration::from_millis(10000), move || {
                        let remaining = pending_count_hide.get().saturating_sub(1);
                        pending_count_hide.set(remaining);

                        if remaining == 0 {
                            badgeh_hide.set_text("");
                            badge_hide.set_text("");
                            app_img_hide.set_from_file(Some("/var/lib/cynager/icons/cos.svg"));
                            cos_btn_hide.remove_css_class("spinning-coin");
                            is_expanded_hide.set(false);

                            let noti_capsule_c   = noti_capsule_hide.clone();
                            let current_width_c = Rc::clone(&current_width_hide);
                            let main_c          = main_window_hide.clone();

                            glib::timeout_add_local(
                                std::time::Duration::from_millis(6),
                                move || {
                                    let next_w = current_width_c.get() - increment_per_frame;
                                    if next_w <= start_width as f64 {
                                        noti_capsule_c.set_width_request(start_width);
                                        noti_capsule_c.remove_css_class("blip");
                                        noti_capsule_c.remove_css_class("notiCapsule");
                                        main_c.set_layer(gtk4_layer_shell::Layer::Top);
                                        main_c.set_namespace(Some("TimeCapsule"));
                                        main_c.set_default_size(-1, -1);
                                        return glib::ControlFlow::Break;
                                    }
                                    current_width_c.set(next_w);
                                    noti_capsule_c.set_width_request(next_w as i32);
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