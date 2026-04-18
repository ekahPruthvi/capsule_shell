use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, Image, Label, Orientation, glib, prelude::*
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::{env, time::Duration};
use chrono::Local;
use gtk4::gio::File;
use std::cell::RefCell;
use std::rc::Rc;
use niri_ipc::{socket::Socket, Action, Request, Response, WorkspaceReferenceArg};

mod notifications;
mod osd;

mod widgets;
use widgets::{battery::spawn_battery_widget, calendar::spawn_calendar_widget};

const HIDE_WORKSPACE_IDX: u8 = 99;

#[derive(Clone)]
struct WindowRecord {
    id: u64,
    workspace_id: Option<u64>,
}

fn send_action(action: Action) {
    if let Ok(mut sock) = Socket::connect() {
        let _ = sock.send(Request::Action(action));
    }
}

fn get_windows() -> Vec<WindowRecord> {
    let Ok(mut sock) = Socket::connect() else { return vec![] };
    match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(windows))) => windows
            .into_iter()
            .map(|w| WindowRecord {
                id: w.id,
                workspace_id: w.workspace_id,
            })
            .collect(),
        _ => vec![],
    }
}

fn makin_widget_window(app: &Application, noti_boxxy: &gtk4::ScrolledWindow){
    let widget_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleN")
        .build();

    widget_window.init_layer_shell();
    widget_window.set_namespace(Some("WidgetScreen"));
    widget_window.set_layer(Layer::Bottom);
    widget_window.set_height_request(100);
    widget_window.remove_css_class("background");
    widget_window.set_anchor(Edge::Bottom, true);
    widget_window.set_anchor(Edge::Top, true);
    widget_window.set_anchor(Edge::Left, true);
    widget_window.set_anchor(Edge::Right, true);
    widget_window.set_exclusive_zone(-1);

    let screen = GtkBox::new(Orientation::Vertical, 5);
    
    screen.append(noti_boxxy);
    widget_window.set_child(Some(&screen));

    spawn_battery_widget();
    spawn_calendar_widget();

    widget_window.present();
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


    let appey = app.clone();
    notifications::connect_notifications_to_dock(rx, &time_capsule, &time_window, &cos_logo, &cos, &badge, &noti_boxy_inner_notifications_all, &appey);
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
        .hexpand(true)
        .halign(gtk4::Align::Fill)
        .vexpand(true)
        .valign(gtk4::Align::End)
        .css_classes(["notiScroller"])               
        .child(&noti_boxy)                        
        .build();

    if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
        let geometry = monitor.geometry();
        let height = geometry.height();
        scrolled_window.set_height_request((height as f64 * 0.1) as i32);
    }
    

    let appy = app.clone();
    makin_widget_window(&appy, &scrolled_window);

    let records: Rc<RefCell<Vec<WindowRecord>>> = Rc::new(RefCell::new(vec![]));
    let is_hidden: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    let records_clone = records.clone();
    let is_hidden_clone = is_hidden.clone();
    let timendate_clone = timendate.clone();

    let show = Image::from_file("/var/lib/cynager/icons/win2.svg");
    show.set_icon_size(gtk4::IconSize::Normal);
    show.set_margin_start(10);
    show.set_margin_end(5);
    time_and_actions.connect_clicked(move |_| {
            let mut hiding = is_hidden_clone.borrow_mut();

            if !*hiding {
                let wins = get_windows();

                for w in &wins {
                    send_action(Action::MoveWindowToWorkspace {
                        window_id: Some(w.id),
                        reference: WorkspaceReferenceArg::Index(HIDE_WORKSPACE_IDX),
                        focus: false
                    });
                }

                *records_clone.borrow_mut() = wins;
                *hiding = true;

                timendate_clone.append(&show);
            } else {
                let wins = records_clone.borrow().clone();
                for w in &wins {
                    let target = match w.workspace_id {
                        Some(id) => WorkspaceReferenceArg::Id(id),
                        None => WorkspaceReferenceArg::Index(1),
                    };
                    send_action(Action::MoveWindowToWorkspace {
                        window_id: Some(w.id),
                        reference: target,
                        focus: false
                    });
                }

                send_action(Action::FocusWorkspace {
                    reference: WorkspaceReferenceArg::Index(1),
                });

                records_clone.borrow_mut().clear();
                *hiding = false;
                timendate_clone.remove(&show);
            }
        });
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}