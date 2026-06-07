use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, CssProvider, EventControllerMotion,
    Image, Label, Orientation, glib, prelude::*,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use std::{env, time::Duration, process::Command};
use chrono::Local;
use gtk4::gio::File;
use std::cell::RefCell;
use std::rc::Rc;
use libc;
use niri_ipc::{socket::Socket, Action, PositionChange, Request, Response, WorkspaceReferenceArg};

mod notifications;
mod osd;
mod ssd;
mod widgets;
mod ctrl;

use widgets::{system::spawn_sys_widget, calendar::spawn_calendar_widget, battery::spawn_bat_widget, stick::spawn_stick_widget, kill};
use ctrl::{spawn_network_watcher, NetworkState, spawn_ctrl_capsules};

#[derive(Debug, Clone, PartialEq)]
struct WidgetConfig {
    cal:      bool,
    sys:      bool,
    shellout: String,
    bat:      bool,
    stick:    bool,
}

impl Default for WidgetConfig {
    fn default() -> Self {
        Self { cal: false, sys: false, shellout: String::new(), bat: false, stick: false }
    }
}

fn parse_widget_config(path: &str) -> Option<WidgetConfig> {
    let content = std::fs::read_to_string(path).ok()?;

    let set_start = content.find(":set")?;
    let set_body  = &content[set_start + 4..];
    let set_end   = set_body.find(":end")?;
    let set_body  = &set_body[..set_end];

    let mut shellout = String::new();
    for line in set_body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("shellout") {
            let rest = rest.trim();
            if let Some(val) = rest.strip_prefix(':') {
                shellout = val.trim().to_string();
                break;
            }
        }
    }

    let w_start      = set_body.find("widgets")?;
    let after_w      = &set_body[w_start + "widgets".len()..];
    let brace_open   = after_w.find('{')? + 1;
    let widget_block = &after_w[brace_open..];
    let brace_close  = widget_block.find('}')?;
    let widget_block = &widget_block[..brace_close];

    let mut cfg = WidgetConfig { shellout, ..Default::default() };
    for line in widget_block.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let line = line.strip_prefix(':').unwrap_or(line);
        let mut parts = line.splitn(2, ':');
        let key = parts.next().map(str::trim).unwrap_or("");
        let val = parts.next().map(str::trim).unwrap_or("false");
        match key {
            "cal"   => cfg.cal   = val == "true",
            "sys"   => cfg.sys   = val == "true",
            "bat"   => cfg.bat   = val == "true",
            "stick" => cfg.stick = val == "true",
            _       => {}
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
    is_floating:  bool,
    float_x:      Option<f64>,
    float_y:      Option<f64>,
    float_w:      i32,
    float_h:      i32,
    corner_x:     Option<f64>,
    corner_y:     Option<f64>,
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
                let (float_x, float_y) = match w.layout.tile_pos_in_workspace_view {
                    Some((x, y)) => (Some(x), Some(y)),
                    None         => (None, None),
                };
                WindowRecord {
                    id:           w.id,
                    workspace_id: w.workspace_id,
                    column_index,
                    row_index,
                    is_floating:  w.is_floating,
                    float_x,
                    float_y,
                    float_w:      w.layout.window_size.0,
                    float_h:      w.layout.window_size.1,
                    corner_x:     None,
                    corner_y:     None,
                }
            })
            .collect(),
        _ => vec![],
    }
}


fn get_focused_output_size() -> Option<(f64, f64)> {
    let Ok(mut sock) = Socket::connect() else { return None };
    match sock.send(Request::FocusedOutput) {
        Ok(Ok(Response::FocusedOutput(Some(output)))) => {
            output.logical.map(|l| (l.width as f64, l.height as f64))
        }
        _ => None,
    }
}

fn corner_hide_target(
    orig_x: f64, orig_y: f64,
    win_w: i32, win_h: i32,
    screen_w: f64, screen_h: f64,
) -> (f64, f64) {
    const PEEK: f64 = 10.0;
    let cx = orig_x + win_w as f64 / 2.0;
    let cy = orig_y + win_h as f64 / 2.0;
    let to_right  = cx > screen_w / 2.0;
    let to_bottom = cy > screen_h / 2.0;
    let tx = if to_right  { screen_w - PEEK } else { PEEK - win_w as f64 };
    let ty = if to_bottom { screen_h - PEEK } else { PEEK - win_h as f64 };
    (tx, ty)
}

fn animate_float_window(
    win_id:  u64,
    from_x:  f64, from_y:  f64,
    to_x:    f64, to_y:    f64,
    on_done: impl Fn() + 'static,
) {
    const STEPS: u32 = 12;
    const TICK_MS: u64 = 16; // ~60 fps
    let step = Rc::new(RefCell::new(0u32));
    glib::timeout_add_local(Duration::from_millis(TICK_MS), move || {
        let s = *step.borrow();
        if s >= STEPS {
            on_done();
            return glib::ControlFlow::Break;
        }
        let progress = s as f64 / STEPS as f64;
        let t = 1.0 - (1.0 - progress).powi(3);
        let x = from_x + (to_x - from_x) * t;
        let y = from_y + (to_y - from_y) * t;
        send_action(Action::MoveFloatingWindow {
            id: Some(win_id),
            x:  PositionChange::SetFixed(x),
            y:  PositionChange::SetFixed(y),
        });
        *step.borrow_mut() += 1;
        glib::ControlFlow::Continue
    });
}

fn network_icon_and_tip(state: &ctrl::NetworkState) -> (&'static str, String) {
    match state {
        NetworkState::WifiConnected(ssid) => (
            "/var/lib/cynager/icons/wifi.svg",
            format!("WiFi: {}", ssid),
        ),
        NetworkState::EthernetConnected(iface) => (
            "/var/lib/cynager/icons/ethernet.svg",
            format!("Ethernet: ({})", iface),
        ),
        NetworkState::NoInternet => (
            "/var/lib/cynager/icons/nointernet.svg",
            "Connected with No Internet".to_string(),
        ),
        NetworkState::Disconnected => (
            "/var/lib/cynager/icons/disconnected.svg",
            "Disconnected".to_string(),
        ),
        NetworkState::WifiOff => (
            "/var/lib/cynager/icons/wifioff.svg",
            "WiFi: off".to_string(),
        ),
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BatteryState {
    percent:  u8,
    charging: bool,
}

fn get_battery_state() -> Option<BatteryState> {
    let entries = std::fs::read_dir("/sys/class/power_supply").ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with("BAT") && !name.starts_with("CMB") {
            continue;
        }
        let base = entry.path();

        let capacity: u8 = std::fs::read_to_string(base.join("capacity"))
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);

        let status = std::fs::read_to_string(base.join("status"))
            .unwrap_or_default();
        let charging = matches!(status.trim(), "Charging" | "Full");

        if capacity < 20 && !charging && !std::path::Path::new("/tmp/batt_no_ask.var").exists() {
            let _ = Command::new("batt_low").status();
        }

        return Some(BatteryState { percent: capacity, charging });
    }
    None
}

fn battery_icon(state: &BatteryState) -> &'static str {
    if state.charging {
        "/var/lib/cynager/icons/battcharge.svg"
    } else {
        match state.percent {
            75..=100 => "/var/lib/cynager/icons/battfull.svg",
            40..=74  => "/var/lib/cynager/icons/batthigh.svg",
            15..=39  => "/var/lib/cynager/icons/battlow.svg",
            _        => "/var/lib/cynager/icons/battempty.svg",
        }
    }
}

fn battery_tip(state: &BatteryState) -> String {
    let status = if state.charging { "Charging" } else { "Discharging" };
    format!("{} · {}%", status, state.percent)
}

fn spawn_battery_watcher(interval: Duration) -> std::sync::mpsc::Receiver<Option<BatteryState>> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<BatteryState>>();
    std::thread::spawn(move || {
        let mut last: Option<Option<BatteryState>> = None;
        loop {
            let state = get_battery_state();
            if Some(&state) != last.as_ref() {
                if tx.send(state.clone()).is_err() { break; }
                last = Some(state);
            }
            std::thread::sleep(interval);
        }
    });
    rx
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
    time_window.remove_css_class("background");
    time_window.set_anchor(Edge::Top, true);
    time_window.set_margin(Edge::Top, 5);
    time_window.set_width_request(400);
    time_window.set_exclusive_zone(0);
    time_window.set_default_size(-1, -1);

    pin_to_monitor(&time_window, mon);

    let time_capsule = GtkBox::new(Orientation::Horizontal, 5);
    time_capsule.set_css_classes(&["timeCapsule", "starting"]);
    time_capsule.set_halign(gtk4::Align::Center);
    time_capsule.set_valign(gtk4::Align::Start);
    time_capsule.set_hexpand(true);
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
        // .hexpand(true)
        // .halign(gtk4::Align::End)
        .build();

    let time_win = time_capsule.clone();
    let time_actual_window = time_window.clone();
    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let now = Local::now();
        time.set_text(&now.format("%I:%M").to_string());
        ampm.set_text(&now.format(" %p \n %a, %b %e").to_string());
        time_win.remove_css_class("starting");
        time_actual_window.set_width_request(300);
        glib::ControlFlow::Continue
    });

    let cos= Button::new();
    let cos_logo = Image::from_file("/var/lib/cynager/icons/cos.svg");
    cos_logo.set_icon_size(gtk4::IconSize::Large);
    cos.set_child(Some(&cos_logo));
    cos.set_css_classes(&["cosIcon"]);
    cos.set_margin_end(15);
    cos_logo.set_cursor_from_name(Some("crosshair"));

    let badge_container = GtkBox::new(Orientation::Vertical, 1);
    badge_container.set_hexpand(true);

    let badge_head = Label::builder()
        .css_classes(["notification_badge"])
        .halign(gtk4::Align::Start)
        .visible(false)
        .label("bruh")
        .build();

    let badge = Label::builder()
        .css_classes(["notification_badge"])
        .halign(gtk4::Align::Start)
        .visible(false)
        .label("")
        .build();
    badge.set_wrap(false);
    badge.set_single_line_mode(true);
    badge.set_max_width_chars(100);
    badge.set_ellipsize(gtk4::pango::EllipsizeMode::End);

    badge_container.append(&badge_head);
    badge_container.append(&badge);

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

    let net_image = Image::from_file("/var/lib/cynager/icons/disconnected.svg");
    net_image.set_icon_size(gtk4::IconSize::Normal);

    let network = Button::new();
    network.set_child(Some(&net_image));
    network.set_css_classes(&["netBtn"]);
    network.set_has_tooltip(true);
    network.set_tooltip_text(Some("Connecting..."));
    

    {
        let net_rx  = spawn_network_watcher(Duration::from_secs(5));
        let net_rx  = Rc::new(RefCell::new(net_rx));
        let img_c   = net_image.clone();
        let btn_c   = network.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            while let Ok(state) = net_rx.borrow().try_recv() {
                let (icon, tip) = network_icon_and_tip(&state);
                img_c.set_from_file(Some(icon));
                btn_c.set_tooltip_text(Some(&tip));
            }
            glib::ControlFlow::Continue
        });
    }

    let initial_bat_state = get_battery_state();
    let has_battery = initial_bat_state.is_some();

    let baty_magy = Image::from_file(
        initial_bat_state
            .as_ref()
            .map(battery_icon)
            .unwrap_or("/var/lib/cynager/icons/battfull.svg"),
    );
    baty_magy.set_icon_size(gtk4::IconSize::Normal);

    let battery = Button::builder()
        .css_classes(["batBtn"])
        .has_tooltip(true)
        .child(&baty_magy)
        .margin_end(5)
        .build();

    if let Some(ref s) = initial_bat_state {
        battery.set_tooltip_text(Some(&battery_tip(s)));
    }

    if has_battery {
        let bat_rx = spawn_battery_watcher(Duration::from_secs(10));
        let bat_rx = Rc::new(RefCell::new(bat_rx));
        let bat_img_c = baty_magy.clone();
        let bat_btn_c = battery.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            while let Ok(state_opt) = bat_rx.borrow().try_recv() {
                if let Some(state) = state_opt {
                    bat_img_c.set_from_file(Some(battery_icon(&state)));
                    bat_btn_c.set_tooltip_text(Some(&battery_tip(&state)));
                }
            }
            glib::ControlFlow::Continue
        });
    }

    let overlay_open: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
 
    {
        let flag  = overlay_open.clone();
        let app_c = app.clone();
        network.connect_clicked(move |_| {
            if *flag.borrow() { return; }
            *flag.borrow_mut() = true;
            spawn_ctrl_capsules(&app_c, flag.clone());
        }); 
    }
 
    if has_battery {
        let flag  = overlay_open.clone();
        let app_c = app.clone();
        battery.connect_clicked(move |_| {
            if *flag.borrow() { return; }
            *flag.borrow_mut() = true;
            spawn_ctrl_capsules(&app_c, flag.clone());
        });
    }

    let dummy_fill = GtkBox::new(Orientation::Horizontal, 0);
    dummy_fill.set_hexpand(true);

    let clippy = GtkBox::new(Orientation::Horizontal, 4);
    clippy.set_width_request(5);
    clippy.set_hexpand(false);
    clippy.set_halign(gtk4::Align::End);

    let clip_hover = EventControllerMotion::new();
    let clippy_enter_clone = clippy.clone();
    clip_hover.connect_enter(move |_, _, _| {
        clippy_enter_clone.add_css_class("clippy");
        clippy_enter_clone.set_width_request(100);
    });
    let clippy_lev_clone = clippy.clone();
    clip_hover.connect_leave(move |_| {
        clippy_lev_clone.remove_css_class("clippy");
        clippy_lev_clone.set_width_request(5);
    });
    clippy.add_controller(clip_hover);

    fn icon_for_path(path: &std::path::Path) -> String {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            // images
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "tiff" | "ico"
                => "image-x-generic".to_string(),
            // video
            "mp4" | "mkv" | "avi" | "mov" | "webm" | "flv" | "wmv"
                => "video-x-generic".to_string(),
            // audio
            "mp3" | "flac" | "ogg" | "wav" | "aac" | "m4a" | "opus"
                => "audio-x-generic".to_string(),
            // documents
            "pdf"  => "application-pdf".to_string(),
            "doc" | "docx" => "application-msword".to_string(),
            "xls" | "xlsx" => "application-vnd.ms-excel".to_string(),
            "ppt" | "pptx" => "application-vnd.ms-powerpoint".to_string(),
            "odt"  => "application-vnd.oasis.opendocument.text".to_string(),
            // text / code
            "txt" | "md" | "rst" => "text-x-generic".to_string(),
            "rs"   => "text-x-rust".to_string(),
            "py"   => "text-x-python".to_string(),
            "js" | "ts" => "text-x-javascript".to_string(),
            "html" | "htm" => "text-html".to_string(),
            "css"  => "text-css".to_string(),
            "c" | "h"  => "text-x-csrc".to_string(),
            "cpp" | "hpp" => "text-x-c++src".to_string(),
            "sh" | "bash" => "application-x-shellscript".to_string(),
            // archives
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar"
                => "application-x-archive".to_string(),
            // executables
            "exe" | "bin" => "application-x-executable".to_string(),
            _ => {
                if path.is_dir() {
                    "folder".to_string()
                } else {
                    "text-x-generic".to_string()
                }
            }
        }
    }

    fn mime_for_path(path: &std::path::Path) -> &'static str {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        match ext.as_str() {
            "png"  => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif"  => "image/gif",
            "webp" => "image/webp",
            "svg"  => "image/svg+xml",
            "bmp"  => "image/bmp",
            "mp4"  => "video/mp4",
            "mkv"  => "video/x-matroska",
            "webm" => "video/webm",
            "mp3"  => "audio/mpeg",
            "ogg"  => "audio/ogg",
            "flac" => "audio/flac",
            "wav"  => "audio/wav",
            "pdf"  => "application/pdf",
            "txt" | "md" => "text/plain",
            "html" | "htm" => "text/html",
            _      => "application/octet-stream",
        }
    }

    fn add_link_to_clippy(clippy: &GtkBox, url: &str) {
        let url_owned = url.to_string();

        let img = Image::new();
        img.set_icon_name(Some("emblem-web"));
        img.set_icon_size(gtk4::IconSize::Large);

        let btn = Button::new();
        btn.set_child(Some(&img));
        btn.set_css_classes(&["clippyFileBtn"]);
        btn.set_has_tooltip(false);

        let hover_ctl = EventControllerMotion::new();
        let btn_enter = btn.clone();
        let url_for_tip = url_owned.clone();
        hover_ctl.connect_enter(move |_, _, _| {
            btn_enter.set_has_tooltip(true);
            btn_enter.set_tooltip_text(Some(&url_for_tip));
        });
        let btn_leave = btn.clone();
        hover_ctl.connect_leave(move |_| {
            btn_leave.set_has_tooltip(false);
        });
        btn.add_controller(hover_ctl);

        // Left-click: copy URL to clipboard
        // let url_for_click = url_owned.clone();
        // let gesture_click = gtk4::GestureClick::new();
        // gesture_click.set_button(1);
        // gesture_click.connect_released(move |_, _, _, _| {
        //     if let Some(display) = gtk4::gdk::Display::default() {
        //         display.clipboard().set_text(&url_for_click);
        //     }
        // });
        // btn.add_controller(gesture_click);

        let drag_src = gtk4::DragSource::new();
        drag_src.set_actions(gtk4::gdk::DragAction::COPY | gtk4::gdk::DragAction::MOVE);

        let url_for_drag = url_owned.clone();
        drag_src.connect_prepare(move |_src, _, _| {
            let uri_list = format!("{}\r\n", url_for_drag);
            let uri_bytes = glib::Bytes::from(uri_list.as_bytes());
            let uri_provider = gtk4::gdk::ContentProvider::for_bytes("text/uri-list", &uri_bytes);

            let text_bytes = glib::Bytes::from(url_for_drag.as_bytes());
            let text_provider = gtk4::gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &text_bytes);
            let text_plain   = gtk4::gdk::ContentProvider::for_bytes("text/plain", &text_bytes);

            Some(gtk4::gdk::ContentProvider::new_union(&[text_plain, text_provider, uri_provider]))
        });

        drag_src.connect_drag_begin(move |src, _drag| {
            let display = gtk4::gdk::Display::default().expect("No display found");
            let theme = gtk4::IconTheme::for_display(&display);
            let paintable = theme.lookup_icon(
                "emblem-web",
                &["application-x-executable"],
                48,
                1,
                gtk4::TextDirection::None,
                gtk4::IconLookupFlags::empty(),
            );
            src.set_icon(Some(&paintable), 24, 24);
        });

        // let btn_for_drag_end = btn.clone();
        // let clippy_for_drag_end = clippy.clone();
        // drag_src.connect_drag_end(move |_, _drag, _delete_data| {
        //     clippy_for_drag_end.remove(&btn_for_drag_end);
        // });
        btn.add_controller(drag_src);

        let gesture_rm = gtk4::GestureClick::new();
        gesture_rm.set_button(3);
        let btn_for_remove = btn.clone();
        let clippy_for_remove = clippy.clone();
        gesture_rm.connect_released(move |_, _, _, _| {
            clippy_for_remove.remove(&btn_for_remove);
        });
        btn.add_controller(gesture_rm);

        clippy.append(&btn);
        clippy.set_visible(true);
    }

    fn add_text_to_clippy(clippy: &GtkBox, text: &str) {
        let text_owned = text.to_string();
        let preview: String = text_owned.chars().take(40).collect();
        let display_label = if text_owned.chars().count() > 40 {
            format!("{}…", preview)
        } else {
            preview.clone()
        };

        let lbl = gtk4::Label::new(Some(&display_label));
        lbl.set_max_width_chars(12);
        lbl.set_ellipsize(gtk4::pango::EllipsizeMode::End);
        lbl.set_single_line_mode(true);

        let btn = Button::new();
        btn.set_child(Some(&lbl));
        btn.set_css_classes(&["clippyFileBtn", "clippyTextBtn"]);
        btn.set_has_tooltip(false);

        let hover_ctl = EventControllerMotion::new();
        let btn_enter = btn.clone();
        let text_for_tip = text_owned.clone();
        hover_ctl.connect_enter(move |_, _, _| {
            btn_enter.set_has_tooltip(true);
            btn_enter.set_tooltip_text(Some(&text_for_tip));
        });
        let btn_leave = btn.clone();
        hover_ctl.connect_leave(move |_| {
            btn_leave.set_has_tooltip(false);
        });
        btn.add_controller(hover_ctl);

        let text_for_click = text_owned.clone();
        let gesture_click = gtk4::GestureClick::new();
        gesture_click.set_button(1);
        gesture_click.connect_released(move |_, _, _, _| {
            if let Some(display) = gtk4::gdk::Display::default() {
                display.clipboard().set_text(&text_for_click);
            }
        });
        btn.add_controller(gesture_click);

        let drag_src = gtk4::DragSource::new();
        drag_src.set_actions(gtk4::gdk::DragAction::COPY | gtk4::gdk::DragAction::MOVE);

        let text_for_drag = text_owned.clone();
        drag_src.connect_prepare(move |_src, _, _| {
            let bytes = glib::Bytes::from(text_for_drag.as_bytes());
            let provider_utf8  = gtk4::gdk::ContentProvider::for_bytes("text/plain;charset=utf-8", &bytes);
            let provider_plain = gtk4::gdk::ContentProvider::for_bytes("text/plain", &bytes);
            // Offer both variants — some webapps (Discord) only accept bare text/plain
            Some(gtk4::gdk::ContentProvider::new_union(&[provider_utf8, provider_plain]))
        });

        drag_src.connect_drag_begin(move |src, _drag| {
            let display = gtk4::gdk::Display::default().expect("No display found");
            let theme = gtk4::IconTheme::for_display(&display);
            let paintable = theme.lookup_icon(
                "text-x-generic",
                &["application-x-executable"],
                48,
                1,
                gtk4::TextDirection::None,
                gtk4::IconLookupFlags::empty(),
            );
            src.set_icon(Some(&paintable), 24, 24);
        });

        // let btn_for_drag_end = btn.clone();
        // let clippy_for_drag_end = clippy.clone();
        // drag_src.connect_drag_end(move |_, _drag, _delete_data| {
        //     clippy_for_drag_end.remove(&btn_for_drag_end);
        // });
        btn.add_controller(drag_src);

        let gesture_rm = gtk4::GestureClick::new();
        gesture_rm.set_button(3);
        let btn_for_remove = btn.clone();
        let clippy_for_remove = clippy.clone();
        gesture_rm.connect_released(move |_, _, _, _| {
            clippy_for_remove.remove(&btn_for_remove);
        });
        btn.add_controller(gesture_rm);

        clippy.append(&btn);
        clippy.set_visible(true);
    }

    fn add_file_to_clippy(clippy: &GtkBox, uri: &str) {
        let path = if let Some(p) = uri.strip_prefix("file://") {
            let decoded = percent_decode(p);
            std::path::PathBuf::from(decoded)
        } else {
            std::path::PathBuf::from(uri)
        };

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(uri)
            .to_string();
        let icon_name = icon_for_path(&path);
        let uri_owned = uri.to_string();

        let img = Image::new();
        img.set_icon_name(Some(&icon_name));
        img.set_icon_size(gtk4::IconSize::Large);

        let btn = Button::new();
        btn.set_child(Some(&img));
        btn.set_css_classes(&["clippyFileBtn"]);
        btn.set_has_tooltip(false); 

        let hover_ctl = EventControllerMotion::new();
        let btn_enter = btn.clone();
        let name_for_tooltip = file_name.clone();
        hover_ctl.connect_enter(move |_, _, _| {
            btn_enter.set_has_tooltip(true);
            btn_enter.set_tooltip_text(Some(&name_for_tooltip));
        });
        let btn_leave = btn.clone();
        hover_ctl.connect_leave(move |_| {
            btn_leave.set_has_tooltip(false);
        });
        btn.add_controller(hover_ctl);

        let drag_src = gtk4::DragSource::new();
        drag_src.set_actions(gtk4::gdk::DragAction::COPY | gtk4::gdk::DragAction::MOVE);

        let uri_for_drag = uri_owned.clone();
        drag_src.connect_prepare(move |_src, _, _| {
            let gfile = gtk4::gio::File::for_uri(&uri_for_drag);
            let gfile_val = glib::Value::from(&gfile);
            let gfile_provider = gtk4::gdk::ContentProvider::for_value(&gfile_val);
            let mime_provider = if let Some(local_path) = uri_for_drag.strip_prefix("file://") {
                let decoded = percent_decode(local_path);
                if let Ok(file_bytes) = std::fs::read(&decoded) {
                    let mime = mime_for_path(std::path::Path::new(&decoded));
                    let gbytes = glib::Bytes::from_owned(file_bytes);
                    Some(gtk4::gdk::ContentProvider::for_bytes(mime, &gbytes))
                } else {
                    None
                }
            } else {
                None
            };

            let uri_list  = format!("{}\r\n", uri_for_drag);
            let uri_bytes = glib::Bytes::from(uri_list.as_bytes());
            let uri_provider = gtk4::gdk::ContentProvider::for_bytes("text/uri-list", &uri_bytes);

            let providers: Vec<gtk4::gdk::ContentProvider> = match mime_provider {
                Some(mp) => vec![gfile_provider, mp, uri_provider],
                None     => vec![gfile_provider, uri_provider],
            };

            Some(gtk4::gdk::ContentProvider::new_union(&providers))
        });

        let icon_name_drag = icon_name.clone();
        drag_src.connect_drag_begin(move |src, _drag| {
            let display = gtk4::gdk::Display::default().expect("No display found");
            let theme = gtk4::IconTheme::for_display(&display);
            let paintable = theme.lookup_icon(
                &icon_name_drag,
                &["application-x-executable"],
                48, 
                1,
                gtk4::TextDirection::None,
                gtk4::IconLookupFlags::empty(),
            );
            src.set_icon(Some(&paintable), 24, 24);
        });

        // let btn_for_drag_end = btn.clone();
        // let clippy_for_drag_end = clippy.clone();
        // drag_src.connect_drag_end(move |_, _drag, _delete_data| {
        //     clippy_for_drag_end.remove(&btn_for_drag_end);
        // });

        btn.add_controller(drag_src);

        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3);
        let btn_for_remove = btn.clone();
        let clippy_for_remove = clippy.clone();
        gesture.connect_released(move |_, _, _, _| {
            clippy_for_remove.remove(&btn_for_remove);
        });
        btn.add_controller(gesture);

        clippy.append(&btn);
        clippy.set_visible(true);
    }

    fn percent_decode(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut bytes = s.bytes().peekable();
        while let Some(b) = bytes.next() {
            if b == b'%' {
                let h1 = bytes.next().unwrap_or(b'?');
                let h2 = bytes.next().unwrap_or(b'?');
                if let Ok(n) = u8::from_str_radix(
                    &format!("{}{}", h1 as char, h2 as char), 16
                ) {
                    out.push(n as char);
                    continue;
                }
            }
            out.push(b as char);
        }
        out
    }

    {
        let drop_target = gtk4::DropTarget::builder()
            .actions(gtk4::gdk::DragAction::COPY | gtk4::gdk::DragAction::MOVE)
            .build();
        drop_target.set_types(&[gtk4::gio::File::static_type(), glib::Type::STRING]);

        let clippy_drop = clippy.clone();
        drop_target.connect_drop(move |_, value, _, _| {
            if let Ok(file) = value.get::<gtk4::gio::File>() {
                let uri = file.uri().to_string();
                if !uri.is_empty() {
                    add_file_to_clippy(&clippy_drop, &uri);
                } else {
                    clippy_drop.add_css_class("nooclip");
                }
                clippy_drop.set_width_request(50);
                return true;
            }
            if let Ok(text) = value.get::<String>() {
                let mut handled = false;
                let tokens: Vec<&str> = text
                    .split(['\n', '\r'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();

                let all_uris = !tokens.is_empty() && tokens.iter().all(|t| {
                    t.starts_with("file://")
                        || t.starts_with("http://")
                        || t.starts_with("https://")
                        || t.starts_with("ftp://")
                        || t.starts_with('/')
                });

                if all_uris {
                    for token in &tokens {
                        if token.starts_with("file://") || token.starts_with('/') {
                            let normalized = if token.starts_with('/') {
                                format!("file://{token}")
                            } else {
                                token.to_string()
                            };
                            add_file_to_clippy(&clippy_drop, &normalized);
                        } else if token.starts_with("http://")
                            || token.starts_with("https://")
                            || token.starts_with("ftp://")
                        {
                            add_link_to_clippy(&clippy_drop, token);
                        }
                        handled = true;
                    }
                } else if !text.trim().is_empty() {
                    add_text_to_clippy(&clippy_drop, text.trim());
                    handled = true;
                }

                if handled {
                    clippy_drop.set_width_request(50);
                    return true;
                }
            }
            false
        });

        let clippy_motion = clippy.clone();
        drop_target.connect_enter(move |_, _, _| {
            clippy_motion.set_width_request(100);
            clippy_motion.add_css_class("clippy");
            gtk4::gdk::DragAction::COPY
        });

        clippy.add_controller(drop_target);
    }

    let c_tna = GtkBox::new(Orientation::Horizontal, 5);

    c_tna.append(&clippy);
    c_tna.append(&time_and_actions);

    time_capsule.append(&cos);
    time_capsule.append(&badge_container);
    time_capsule.append(&c_tna);
    time_capsule.append(&network);
    if has_battery {
        time_capsule.append(&battery);
    }

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

    let noti_panel_window = ApplicationWindow::builder()
        .application(app)
        .title("capsuleN")
        .build();

    noti_panel_window.init_layer_shell();
    noti_panel_window.set_namespace(Some("Notifications"));
    noti_panel_window.set_layer(Layer::Bottom);
    noti_panel_window.remove_css_class("background");
    noti_panel_window.set_anchor(Edge::Bottom, true);
    noti_panel_window.set_exclusive_zone(-1);
    noti_panel_window.set_default_size(-1, -1);

    pin_to_monitor(&noti_panel_window, mon);

    noti_panel_window.set_child(Some(&scrolled_window));
    noti_panel_window.present();

    notifications::connect_notifications_to_dock(
        rx, &time_capsule, &time_window, &cos_logo, &cos, &badge, &badge_head,
        &noti_boxy_inner_notifications_all,
    );
    osd::connect_osd_to_dock(&osd, &osd_revealer, &osd_capsule, &osd_window, &lbl);

    time_window.present();

    let active_cal: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.cal));
    let active_sys: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.sys));
    let active_bat: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.bat));
    let active_stick: Rc<RefCell<bool>> = Rc::new(RefCell::new(initial_cfg.stick));

    let cal_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.cal {
        *cal_win.borrow_mut() = Some(spawn_calendar_widget(shellout_monitor.as_ref()));
    }
    let sys_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.sys {
        *sys_win.borrow_mut() = Some(spawn_sys_widget(shellout_monitor.as_ref()));
    }
    let stick_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.stick {
        *stick_win.borrow_mut() = Some(spawn_stick_widget(shellout_monitor.as_ref()));
    }
    let bat_win: Rc<RefCell<Option<gtk4::Window>>> = Rc::new(RefCell::new(None));
    if initial_cfg.bat {
        *bat_win.borrow_mut() = Some(spawn_bat_widget(shellout_monitor.as_ref()));
    }

    let probe_rx     = spawn_probe_watcher(probe_path.to_string(), Duration::from_secs(5));
    let probe_rx     = Rc::new(RefCell::new(probe_rx));
    let active_cal_c = active_cal.clone();
    let active_sys_c = active_sys.clone();
    let active_bat_c = active_bat.clone();
    let active_stick_c = active_stick.clone();
    let pp_monitor = shellout_monitor.clone();

    glib::timeout_add_local(Duration::from_millis(500), move || {
        while let Ok(cfg) = probe_rx.borrow().try_recv() {
            let cal_active = *active_cal_c.borrow();
            if cfg.cal && !cal_active {
                *cal_win.borrow_mut() = Some(spawn_calendar_widget(pp_monitor.as_ref()));
                *active_cal_c.borrow_mut() = true;
            } else if !cfg.cal && cal_active {
                let maybe = cal_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_cal_c.borrow_mut() = false;
            }

            let sys_active = *active_sys_c.borrow();
            if cfg.sys && !sys_active {
                *sys_win.borrow_mut() = Some(spawn_sys_widget(pp_monitor.as_ref()));
                *active_sys_c.borrow_mut() = true;
            } else if !cfg.sys && sys_active {
                let maybe = sys_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_sys_c.borrow_mut() = false;
            }

            let bat_active = *active_bat_c.borrow();
            if cfg.bat && !bat_active {
                *bat_win.borrow_mut() = Some(spawn_bat_widget(pp_monitor.as_ref()));
                *active_bat_c.borrow_mut() = true;
            } else if !cfg.bat && bat_active {
                let maybe = bat_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_bat_c.borrow_mut() = false;
            }

            let stick_active = *active_stick_c.borrow();
            if cfg.stick && !stick_active {
                *stick_win.borrow_mut() = Some(spawn_stick_widget(pp_monitor.as_ref()));
                *active_stick_c.borrow_mut() = true;
            } else if !cfg.stick && stick_active {
                let maybe = stick_win.borrow_mut().take();
                if let Some(w) = maybe { kill(&w); }
                *active_stick_c.borrow_mut() = false;
            }
        }
        glib::ControlFlow::Continue
    });

    let records:             Rc<RefCell<Vec<WindowRecord>>> = Rc::new(RefCell::new(vec![]));
    let is_hidden:           Rc<RefCell<bool>>              = Rc::new(RefCell::new(false));
    let focused_before_hide: Rc<RefCell<Option<u64>>>       = Rc::new(RefCell::new(None));

    let records_clone  = records.clone();
    let is_hidden_clone = is_hidden.clone();
    // let timendate_clone = timendate.clone(); this is for focus time mode
    let focused_clone  = focused_before_hide.clone();

    let show = Image::from_file("/var/lib/cynager/icons/min.svg");
    show.set_icon_size(gtk4::IconSize::Normal);
    show.set_margin_start(10);
    show.set_margin_end(5);

    glib::unix_signal_add_local(libc::SIGUSR1, move || {
        let mut hiding = is_hidden_clone.borrow_mut();

        if !*hiding {
            let wins = get_windows();
            *focused_clone.borrow_mut() = get_focused_window_id();
            *records_clone.borrow_mut() = wins.clone();
            *hiding = true;
            // timendate_clone.append(&show);

            let screen = get_focused_output_size().unwrap_or((1920.0, 1080.0));

            let mut pending = wins.len();
            let records_c2 = records_clone.clone();

            for w in wins {
                if w.is_floating {
                    let orig_x = w.float_x.unwrap_or(0.0);
                    let orig_y = w.float_y.unwrap_or(0.0);
                    let (tx, ty) = corner_hide_target(
                        orig_x, orig_y, w.float_w, w.float_h, screen.0, screen.1,
                    );
                    let wid = w.id;
                    let wspace = w.workspace_id;
                    let rc = records_c2.clone();
                    animate_float_window(wid, orig_x, orig_y, tx, ty, move || {
                        let mut recs = rc.borrow_mut();
                        if let Some(rec) = recs.iter_mut().find(|r| r.id == wid) {
                            rec.corner_x = Some(tx);
                            rec.corner_y = Some(ty);
                        }
                        let _ = (wid, wspace);
                    });
                }
                let _ = pending;
                pending = pending.saturating_sub(1);
            }

        } else {
            let wins = records_clone.borrow().clone();
            let screen = get_focused_output_size().unwrap_or((1920.0, 1080.0));

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
                .filter(|w| w.column_index.is_some() && !w.is_floating)
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

            for w in wins.iter().filter(|w| w.is_floating) {
                let orig_x   = w.float_x.unwrap_or(screen.0 * 0.25);
                let orig_y   = w.float_y.unwrap_or(screen.1 * 0.25);
                let corner_x = w.corner_x.unwrap_or(orig_x);
                let corner_y = w.corner_y.unwrap_or(orig_y);

                let wid = w.id;
                send_action(Action::MoveFloatingWindow {
                    id: Some(wid),
                    x:  PositionChange::SetFixed(corner_x),
                    y:  PositionChange::SetFixed(corner_y),
                });
                animate_float_window(wid, corner_x, corner_y, orig_x, orig_y, || {});
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
            // timendate_clone.remove(&show);
        }

        glib::ControlFlow::Continue
    });

    ssd::spawn_shelly_side_decorations(app);
}

fn main() {
    let app = Application::new(Some("ekah.scu.cynideshell"), Default::default());
    app.connect_activate(coping_with);
    app.run();
}