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

use widgets::{system::spawn_sys_widget, calendar::spawn_calendar_widget, battery::spawn_bat_widget, kill};

const HIDE_WORKSPACE_IDX: u8 = 99;

#[derive(Debug, Clone, PartialEq)]
struct WidgetConfig {
    cal:      bool,
    sys:      bool,
    shellout: String,
    bat:      bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self { cal: false, sys: false, shellout: String::new(), bat: false }
    }
}

fn parse_widget_config(path: &str) -> Option<WidgetConfig> {
    let content = std::fs::read_to_string(path).ok()?;

    let set_start  = content.find(":set")?;
    let set_body   = &content[set_start..];
    let set_end    = set_body.find(":end")?;
    let set_body   = &set_body[..set_end];

    let mut shellout = String::new();
    for line in set_body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("shellout") {
            // format: `shellout :eDP-1`
            if let Some(val) = rest.trim().strip_prefix(':') {
                shellout = val.trim().to_string();
                break;
            }
        }
    }

    let w_start     = set_body.find("widgets")?.saturating_add("widgets".len());
    let brace_open  = set_body[w_start..].find(':')?.saturating_add(w_start + 1);
    let brace_open  = set_body[brace_open..].find('{')?.saturating_add(brace_open + 1);
    let brace_close = set_body[brace_open..].find('}')?.saturating_add(brace_open);
    let widget_block = &set_body[brace_open..brace_close];

    let mut cfg = WidgetConfig { shellout, ..Default::default() };
    for line in widget_block.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(2, ':');
        let key = parts.next().map(str::trim).unwrap_or("");
        let val = parts.next().map(str::trim).unwrap_or("false");
        match key {
            "cal" => cfg.cal = val == "true",
            "sys" => cfg.sys = val == "true",
            "bat" => cfg.bat = val == "true",
            _     => {}
        }
    }
    Some(cfg)
}

fn spawn_probe_watcher(
    probe_path: String,
    interval:   Duration,
) -> std::sync::mpsc::Receiver<WidgetConfig> {
    let (sender, receiver) = std::sync::mpsc::channel::<WidgetConfig>();
    std::thread::spawn(move || {
        let mut last: Option<WidgetConfig> = None;
        loop {
            let cfg = parse_widget_config(&probe_path).unwrap_or_default();
            if Some(&cfg) != last.as_ref() {
                if sender.send(cfg.clone()).is_err() { break; }
                last = Some(cfg);
            }
            std::thread::sleep(interval);
        }
    });
    receiver
}

fn resolve_monitor(
    display:   &gtk4::gdk::Display,
    connector: &str,
) -> Option<gtk4::gdk::Monitor> {
    if connector.is_empty() || connector == "default" {
        return None;
    }
    let monitors = display.monitors();
    (0..monitors.n_items())
        .filter_map(|i| monitors.item(i)?.downcast::<gtk4::gdk::Monitor>().ok())
        .find(|m| m.connector().map(|c| c == connector).unwrap_or(false))
}

fn pin_to_monitor(window: &ApplicationWindow, monitor: Option<&gtk4::gdk::Monitor>) {
    if let Some(m) = monitor {
        window.set_monitor(Some(m));
    }
}

#[derive(Clone)]
struct WindowRecord {
    id:           u64,
    workspace_id: Option<u64>,
    column_index: Option<usize>,
    row_index:    Option<usize>,
}

fn get_focused_window_id() -> Option<u64> {
    let Ok(mut sock) = Socket::connect() else { return None };
    match sock.send(Request::FocusedWindow) {
        Ok(Ok(Response::FocusedWindow(Some(w)))) => Some(w.id),
        _ => None,
    }
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
            .map(|w| {
                let (column_index, row_index) = match w.layout.pos_in_scrolling_layout {
                    Some((col, row)) => (Some(col), Some(row)),
                    None             => (None, None),
                };
                WindowRecord { id: w.id, workspace_id: w.workspace_id, column_index, row_index }
            })
            .collect(),
        _ => vec![],
    }
}


fn makin_widget_window(
    app:     &Application,
    boxxy:   &gtk4::ScrolledWindow,
    monitor: Option<&gtk4::gdk::Monitor>,
) {
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

    pin_to_monitor(&noti_window, monitor);

    noti_window.set_child(Some(boxxy));
    noti_window.present();
}


fn coping_with(app: &Application) {
    let rx = notifications::spawn_messaging_daemon();

    let css      = CssProvider::new();
    let home_dir = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let css_path = format!("{}/.config/capsule/dark.css", home_dir);
    css.load_from_file(&File::for_path(&css_path));
    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let probe_path = "/var/lib/cynager/info.probe";
    let initial_cfg = parse_widget_config(probe_path).unwrap_or_default();

    let display = gtk4::gdk::Display::default().expect("Could not get default display");
    let shellout_monitor = resolve_monitor(&display, &initial_cfg.shellout);
    let mon = shellout_monitor.as_ref();

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

    pin_to_monitor(&time_window, mon);

    let time_capsule = GtkBox::new(Orientation::Horizontal, 5);
    time_capsule.set_css_classes(&["timeCapsule", "starting"]);
    time_capsule.set_halign(gtk4::Align::Center);
    time_capsule.set_valign(gtk4::Align::Start);
    time_capsule.set_hexpand(true);
    time_capsule.set_margin_top(5);
    time_capsule.set_margin_bottom(5);
    time_capsule.set_width_request(300);

    let timendate = GtkBox::new(Orientation::Horizontal, 5);
    let time      = Label::new(Some(""));
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

    let time_win           = time_capsule.clone();
    let time_actual_window = time_window.clone();
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let now = Local::now();
        time.set_text(&now.format("%I:%M").to_string());
        ampm.set_text(&now.format(" %p \n %a, %b %e").to_string());
        time_win.remove_css_class("starting");
        time_actual_window.set_width_request(300);
        glib::ControlFlow::Continue
    });

    let cos      = Button::new();
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

    let osd_box = GtkBox::new(Orientation::Vertical, 5);
    osd_box.set_hexpand(true);
    osd_box.set_halign(gtk4::Align::Center);
    osd_box.set_margin_bottom(50);
    osd_box.set_css_classes(&["osdBox"]);

    let osd = GtkBox::new(Orientation::Horizontal, 5);
    osd.set_hexpand(false);
    osd.set_halign(gtk4::Align::Start);
    osd.set_vexpand(false);
    osd.set_width_request(8);

    let osd_revealer = gtk4::Revealer::new();
    osd_revealer.set_transition_type(gtk4::RevealerTransitionType::Crossfade);
    osd_revealer.set_transition_duration(150);
    osd_revealer.set_child(Some(&osd));
    osd_revealer.set_reveal_child(false);
    osd_revealer.set_width_request(300);
    osd_revealer.set_visible(false);

    let lbl = gtk4::Label::new(Some("dummy"));
    lbl.set_hexpand(true);
    lbl.set_halign(gtk4::Align::Start);
    lbl.set_css_classes(&["osdLabel"]);

    osd_box.append(&lbl);
    osd_box.append(&osd_revealer);

    time_capsule.append(&cos);
    time_capsule.append(&badge);
    time_capsule.append(&time_and_actions);
    time_window.set_child(Some(&time_capsule));

    let noti_boxy_inner_notifications_all = GtkBox::new(Orientation::Horizontal, 0);

    let osd_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleO")
        .build();

    osd_window.init_layer_shell();
    osd_window.set_namespace(Some("OSD"));
    osd_window.set_layer(Layer::Overlay);
    osd_window.remove_css_class("background");
    osd_window.set_anchor(Edge::Bottom, true);
    osd_window.set_exclusive_zone(-1);

    let osd_capsule = GtkBox::new(Orientation::Vertical, 5);
    osd_capsule.set_css_classes(&["osdCapsule"]);
    osd_capsule.set_halign(gtk4::Align::Center);
    osd_capsule.set_valign(gtk4::Align::Baseline);
    osd_capsule.set_hexpand(true);
    osd_capsule.set_margin_top(5);
    osd_capsule.set_margin_bottom(0);
    osd_capsule.set_width_request(50);
    osd_capsule.set_height_request(58);

    osd_capsule.append(&osd_box);
    osd_window.set_child(Some(&osd_capsule));

    notifications::connect_notifications_to_dock(
        rx, &time_capsule, &time_window, &cos_logo, &cos, &badge,
        &noti_boxy_inner_notifications_all,
    );
    osd::connect_osd_to_dock(&osd, &osd_revealer, &osd_capsule, &osd_window, &lbl);

    time_window.present();

    let noti_boxy = GtkBox::new(Orientation::Vertical, 0);
    noti_boxy.append(&noti_boxy_inner_notifications_all);
    noti_boxy.set_css_classes(&["notificationWindow"]);
    noti_boxy.set_margin_bottom(10);
    noti_boxy.set_width_request(300);
    noti_boxy.set_halign(gtk4::Align::Center);

    let monitors = display.monitors();
    let scrolled_window = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .css_classes(["notiScroller"])
        .child(&noti_boxy)
        .build();

    let fallback_monitor = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>();
    let width_monitor: Option<&gtk4::gdk::Monitor> =
        mon.or_else(|| fallback_monitor.as_ref().map(|m| m));
    if let Some(m) = width_monitor {
        scrolled_window.set_width_request(m.geometry().width());
    }

    makin_widget_window(app, &scrolled_window, mon);

    let active_cal: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.cal));
    let active_sys: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.sys));
    let active_bat: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.bat));

    let cal_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.cal {
        *cal_win.borrow_mut() = Some(spawn_calendar_widget());
    }
    let sys_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.sys {
        *sys_win.borrow_mut() = Some(spawn_sys_widget());
    }
    let bat_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.bat {
        *bat_win.borrow_mut() = Some(spawn_bat_widget());
    }

    let probe_rx     = spawn_probe_watcher(probe_path.to_string(), Duration::from_secs(5));
    let probe_rx     = Rc::new(RefCell::new(probe_rx));
    let active_cal_c = active_cal.clone();
    let active_sys_c = active_sys.clone();
    let active_bat_c = active_bat.clone();

    glib::timeout_add_local(Duration::from_millis(500), move || {
        while let Ok(cfg) = probe_rx.borrow().try_recv() {
            let cal_active = *active_cal_c.borrow();
            if cfg.cal && !cal_active {
                *cal_win.borrow_mut() = Some(spawn_calendar_widget());
                *active_cal_c.borrow_mut() = true;
            } else if !cfg.cal && cal_active {
                let maybe = cal_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_cal_c.borrow_mut() = false;
            }

            let sys_active = *active_sys_c.borrow();
            if cfg.sys && !sys_active {
                *sys_win.borrow_mut() = Some(spawn_sys_widget());
                *active_cal_c.borrow_mut() = true;
            } else if !cfg.sys && sys_active {
                let maybe = sys_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_sys_c.borrow_mut() = false;
            }

            let bat_active = *active_bat_c.borrow();
            if cfg.bat && !bat_active {
                *bat_win.borrow_mut() = Some(spawn_bat_widget());
                *active_cal_c.borrow_mut() = true;
            } else if !cfg.bat && bat_active {
                let maybe = bat_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_bat_c.borrow_mut() = false;
            }
        }
        glib::ControlFlow::Continue
    });

    let records:             Rc<RefCell<Vec<WindowRecord>>> = Rc::new(RefCell::new(vec![]));
    let is_hidden:           Rc<RefCell<bool>>              = Rc::new(RefCell::new(false));
    let focused_before_hide: Rc<RefCell<Option<u64>>>       = Rc::new(RefCell::new(None));

    let records_clone  = records.clone();
    let is_hidden_clone = is_hidden.clone();
    let timendate_clone = timendate.clone();
    let focused_clone  = focused_before_hide.clone();

    let show = Image::from_file("/var/lib/cynager/icons/min.svg");
    show.set_icon_size(gtk4::IconSize::Normal);
    show.set_margin_start(10);
    show.set_margin_end(5);

    time_and_actions.connect_clicked(move |_| {
        let mut hiding = is_hidden_clone.borrow_mut();

        if !*hiding {
            let wins = get_windows();
            *focused_clone.borrow_mut() = get_focused_window_id();

            for w in &wins {
                send_action(Action::MoveWindowToWorkspace {
                    window_id: Some(w.id),
                    reference: WorkspaceReferenceArg::Index(HIDE_WORKSPACE_IDX),
                    focus:     false,
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
                    None     => WorkspaceReferenceArg::Index(1),
                };
                send_action(Action::MoveWindowToWorkspace {
                    window_id: Some(w.id),
                    reference: target,
                    focus:     false,
                });
            }

            let mut tiled: Vec<&WindowRecord> = wins
                .iter()
                .filter(|w| w.column_index.is_some())
                .collect();
            tiled.sort_by_key(|w| (w.column_index.unwrap(), w.row_index.unwrap_or(1)));

            let mut current_col: Option<usize> = None;
            for w in &tiled {
                let col = w.column_index.unwrap();
                if current_col != Some(col) {
                    current_col = Some(col);
                    send_action(Action::FocusWindow { id: w.id });
                    send_action(Action::MoveColumnToIndex { index: col });
                } else {
                    let row = w.row_index.unwrap_or(1);
                    send_action(Action::FocusWindow { id: w.id });
                    send_action(Action::ConsumeWindowIntoColumn {});
                    for _ in 1..row {
                        send_action(Action::MoveWindowUp {});
                    }
                }
            }

            send_action(Action::FocusWorkspace {
                reference: WorkspaceReferenceArg::Index(1),
            });
            if let Some(fid) = *focused_clone.borrow() {
                send_action(Action::FocusWindow { id: fid });
            }

            records_clone.borrow_mut().clear();
            *focused_clone.borrow_mut() = None;
            *hiding = false;
            timendate_clone.remove(&show);
        }
    });

    let quick_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleQ")
        .css_classes(["quickWindow"])
        .build();

    quick_window.init_layer_shell();
    quick_window.set_namespace(Some("QuickCapsule"));
    quick_window.set_layer(Layer::Top);
    quick_window.set_width_request(30);
    quick_window.remove_css_class("background");
    quick_window.set_anchor(Edge::Left, true);
    quick_window.set_exclusive_zone(0);
    quick_window.set_height_request(400);


    pin_to_monitor(&quick_window, mon);

    let quick_capsule = GtkBox::new(Orientation::Horizontal, 5);
    quick_capsule.set_css_classes(&["timeCapsule", "starting"]);
    quick_capsule.set_halign(gtk4::Align::Start);
    quick_capsule.set_valign(gtk4::Align::Center);
    quick_capsule.set_vexpand(true);
    quick_capsule.set_margin_start(5);
    quick_capsule.set_margin_end(5);
    quick_capsule.set_height_request(300);

    quick_capsule.append(&Label::new(Some("b")));
    
    quick_window.set_child(Some(&quick_capsule));

    // quick_window.present();
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}