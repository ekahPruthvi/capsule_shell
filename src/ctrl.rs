use gtk4::{
    Application, ApplicationWindow, Label, Box as GtkBox, Button, Orientation, prelude::*,
    DrawingArea, gdk_pixbuf::Pixbuf, Image, EventControllerScroll, EventControllerScrollFlags,
};
use gtk4::glib;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::time::Duration;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkState {
    WifiConnected(String),       
    EthernetConnected(String),   
    NoInternet,                 
    Disconnected,
    WifiOff,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundState {
    pub volume:  u32,
    pub muted:   bool,
    pub sink:    String,
}

fn get_sound_state() -> SoundState {
    let wpctl = std::process::Command::new("wpctl")
        .args(["get-volume", "@DEFAULT_AUDIO_SINK@"])
        .output();

    let (volume, muted) = if let Ok(out) = wpctl {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let is_muted = text.contains("[MUTED]");
        let vol = text
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| (v * 100.0).round() as u32)
            .unwrap_or(0);
        (vol, is_muted)
    } else {
        (0, false)
    };

    let sink = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .and_then(|o| {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if raw.is_empty() { return None; }

            let list = std::process::Command::new("pactl")
                .args(["list", "sinks"])
                .output()
                .ok()?;
            let list_text = String::from_utf8_lossy(&list.stdout).to_string();

            let mut in_sink  = false;
            let mut desc: Option<String> = None;
            for line in list_text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Name:") {
                    in_sink = trimmed.contains(&raw);
                }
                if in_sink {
                    if let Some(d) = trimmed.strip_prefix("Description:") {
                        desc = Some(d.trim().to_string());
                        break;
                    }
                }
            }
            desc.or(Some(raw))
        })
        .unwrap_or_else(|| "Unknown Output".to_string());

    SoundState { volume, muted, sink }
}

fn sound_icon(state: &SoundState) -> &'static str {
    if state.muted || state.volume == 0 {
        "/var/lib/cynager/icons/soundmute.svg"
    } else if state.volume <= 33 {
        "/var/lib/cynager/icons/soundlow.svg"
    } else if state.volume <= 66 {
        "/var/lib/cynager/icons/soundmed.svg"
    } else {
        "/var/lib/cynager/icons/soundhigh.svg"
    }
}

pub fn spawn_sound_watcher(interval: Duration) -> std::sync::mpsc::Receiver<SoundState> {
    let (tx, rx) = std::sync::mpsc::channel::<SoundState>();
    std::thread::spawn(move || {
        let mut last: Option<SoundState> = None;
        loop {
            let state = get_sound_state();
            if Some(&state) != last.as_ref() {
                if tx.send(state.clone()).is_err() { break; }
                last = Some(state);
            }
            std::thread::sleep(interval);
        }
    });
    rx
}

fn wifi_soft_blocked() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/class/rfkill") else { return false };
    for entry in entries.flatten() {
        let base = entry.path();
        let type_path = base.join("type");
        let soft_path = base.join("soft");
        if std::fs::read_to_string(&type_path)
            .map(|t| t.trim() == "wlan")
            .unwrap_or(false)
        {
            if std::fs::read_to_string(&soft_path)
                .map(|s| s.trim() == "1")
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

fn has_internet() -> bool {
    let route_ok = std::fs::read_to_string("/proc/net/route")
        .map(|content| {
            content.lines().skip(1).any(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                cols.len() >= 2 && cols[1] == "00000000"
            })
        })
        .unwrap_or(false);

    if !route_ok {
        return false;
    }

    std::net::TcpStream::connect_timeout(
        &"1.1.1.1:53".parse().unwrap(),
        Duration::from_secs(2),
    )
    .is_ok()
}

fn wifi_ssid(iface: &str) -> Option<String> {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID", "dev", "wifi"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(ssid) = line.strip_prefix("yes:") {
                let s = ssid.trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
    }

    if let Ok(out) = std::process::Command::new("iw")
        .args(["dev", iface, "link"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            if let Some(ssid) = line.strip_prefix("SSID:") {
                let s = ssid.trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
    }

    if let Ok(out) = std::process::Command::new("iwgetid")
        .args([iface, "-r"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() { return Some(s); }
    }

    None
}

fn get_network_state() -> NetworkState {
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return NetworkState::Disconnected;
    };

    let mut wifi_up:     Option<String> = None;
    let mut eth_up:      Option<String> = None;
    let mut wifi_exists: bool           = false;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" { continue; }

        let operstate = std::fs::read_to_string(
            format!("/sys/class/net/{}/operstate", name),
        )
        .unwrap_or_default();
        let is_up = operstate.trim() == "up";

        let is_wireless = entry.path().join("wireless").exists();

        if is_wireless {
            wifi_exists = true;
            if is_up { wifi_up = Some(name.clone()); }
        } else if name.starts_with('e') && is_up {
            eth_up = Some(name.clone());
        }
    }

    if wifi_exists && wifi_up.is_none() && wifi_soft_blocked() {
        return NetworkState::WifiOff;
    }

    let connected = wifi_up.is_some() || eth_up.is_some();
    if !connected {
        return NetworkState::Disconnected;
    }

    if !has_internet() {
        return NetworkState::NoInternet;
    }

    if let Some(ref iface) = wifi_up {
        let ssid = wifi_ssid(iface).unwrap_or_else(|| iface.clone());
        return NetworkState::WifiConnected(ssid);
    }

    NetworkState::EthernetConnected(eth_up.unwrap())
}

pub fn spawn_network_watcher(interval: Duration) -> std::sync::mpsc::Receiver<NetworkState> {
    let (tx, rx) = std::sync::mpsc::channel::<NetworkState>();
    std::thread::spawn(move || {
        let mut last: Option<NetworkState> = None;
        loop {
            let state = get_network_state();
            if Some(&state) != last.as_ref() {
                if tx.send(state.clone()).is_err() { break; }
                last = Some(state);
            }
            std::thread::sleep(interval);
        }
    });
    rx
}

fn network_icon_and_tip(state: NetworkState) -> (&'static str, String, String) {
    match state {
        NetworkState::WifiConnected(ssid) => (
            "/var/lib/cynager/icons/wifi.svg",
            "Wifi".to_string(),
            ssid,
        ),
        NetworkState::EthernetConnected(iface) => (
            "/var/lib/cynager/icons/ethernet.svg",
            "Ethernet".to_string(), 
            iface,
        ),
        NetworkState::NoInternet => (
            "/var/lib/cynager/icons/nointernet.svg",
            "Connected".to_string(),
            "with No Internet".to_string(),
        ),
        NetworkState::Disconnected => (
            "/var/lib/cynager/icons/disconnected.svg",
            "Disconnected".to_string(),
            "Connect to a network?".to_string(),
        ),
        NetworkState::WifiOff => (
            "/var/lib/cynager/icons/wifioff.svg",
            "Network OFF".to_string(),
            "Turn on Network?".to_string(),
        ),
    }
}

fn toggle_wifi_adapter(enable: bool) {
    let action = if enable { "unblock" } else { "block" };
    let _ = std::process::Command::new("rfkill")
        .args([action, "wifi"])
        .spawn();
}

fn get_wifi_networks() -> Vec<(String, String, bool)> {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"])
        .output()
    {
        let mut nets: Vec<(String, String, bool)> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, ':').collect();
                if parts.len() >= 3 {
                    let active = parts[0] == "yes";
                    let ssid = parts[1].trim().to_string();
                    if ssid.is_empty() { return None; }
                    let signal: u32 = parts[2].trim().parse().unwrap_or(0);
                    let bars = match signal {
                        0..=20  => "▌",
                        21..=40 => "▌ ▌",
                        41..=60 => "▌ ▌ ▌",
                        _       => "▌ ▌ ▌ ▌",
                    };
                    Some((ssid, bars.to_string(), active))
                } else { None }
            })
            .collect();
        nets.sort_by(|a, b| b.2.cmp(&a.2));
        return nets;
    }
    vec![]
}

pub fn spawn_ctrl_capsules(
    app:          &Application,
    overlay_open: Rc<RefCell<bool>>,
) {
    let win = ApplicationWindow::builder()
        .application(app)
        .title("capsuleCTRL")
        .css_classes(["ctrlOverlay"])
        .build();
 
    win.init_layer_shell();
    win.set_namespace(Some("CtrlOverlay"));
    win.set_layer(Layer::Top);
    win.remove_css_class("background");
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Bottom,true);
    win.set_anchor(Edge::Left,true);
    win.set_anchor(Edge::Right,true);
    // win.set_exclusive_zone(200);
    // win.auto_exclusive_zone_enable();
 
    let backdrop = Button::builder()
        .css_classes(["ctrlBackdrop"])
        .hexpand(true)
        .vexpand(true)
        .build();
    
    let dir_path = "/usr/share/octobacillus/";
    let base_name = "usericon.";
    let valid_extensions = ["png", "jpeg", "jpg"];

    let mut final_path = String::from("/usr/share/octobacillus/usericon.png"); 

    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with(base_name) {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                            final_path = path.to_string_lossy().into_owned();
                            break;
                        }
                    }
                }
            }
        }
    }


    let pixbuf = Pixbuf::from_file(&final_path).unwrap();
    let size = 35;

    let usricon = DrawingArea::new();
    usricon.set_content_width(size);
    usricon.set_content_height(size);

        usricon.set_draw_func(move |_, cr, w, h| {
        let w = w as f64;
        let h = h as f64;
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r  = w / 2.0;
 
        cr.arc(cx, cy, r, 0.0, 2.0 * std::f64::consts::PI);
        cr.clip();
 
        let pb = pixbuf.scale_simple(w as i32, h as i32, gtk4::gdk_pixbuf::InterpType::Bilinear).unwrap();
        cr.set_source_pixbuf(&pb, 0.0, 0.0);
        cr.paint().unwrap();
 
        let shine = gtk4::cairo::LinearGradient::new(
            cx * 0.35, cy * 0.10,
            cx * 0.80, cy * 0.75,
        );
        shine.add_color_stop_rgba(0.00, 1.0, 1.0, 1.0, 0.55);
        shine.add_color_stop_rgba(0.40, 1.0, 1.0, 1.0, 0.18);
        shine.add_color_stop_rgba(1.00, 1.0, 1.0, 1.0, 0.00);
 
        cr.set_source(&shine).unwrap();
 
        cr.save().unwrap();
        cr.translate(cx, cy);
        cr.scale(r * 0.85, r * 0.55);
        cr.translate(-r * 0.08, -r * 0.80);
        cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.restore().unwrap();
        cr.fill().unwrap();
 
        let rim = gtk4::cairo::LinearGradient::new(cx * 0.4, 0.0, cx * 1.6, r * 0.18);
        rim.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.00);
        rim.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.45);
        rim.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.00);
        cr.set_source(&rim).unwrap();
        cr.arc(cx, cy, r - 0.5, std::f64::consts::PI * 1.15, std::f64::consts::PI * 1.85);
        cr.set_line_width(1.5);
        cr.stroke().unwrap();
    });

    
    let usrname = match std::fs::read_to_string("/usr/share/octobacillus/user.octo") {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error reading file: {}", err);
            "name = user4.0".to_string()
        }
    };

    let name = usrname
        .lines()
        .find(|line| line.trim().starts_with("name"))
        .and_then(|line| line.split_once("="))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_else(|| "user4.0".to_string());

    let usrbox = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    usrbox.append(&usricon);

    let usr_labels = GtkBox::new(Orientation::Vertical, 2);
    usr_labels.append(&Label::builder()
        .label("Hello,")
        .css_classes(["userHello"])
        .halign(gtk4::Align::Start)
        .build()
    );
    usr_labels.append(&Label::builder()
        .label(name)
        .css_classes(["userName"])
        .halign(gtk4::Align::Start)
        .build()
    );

    usrbox.append(&usr_labels);
    
    let usr = Button::builder()
        .child(&usrbox)
        .css_classes(["ctrlBtnL"])
        .build();


    let net_rx = spawn_network_watcher(Duration::from_secs(5));
    let initial_state = get_network_state();
    let (init_icon, init_label, init_body) = network_icon_and_tip(initial_state);

    let net_icon  = Image::from_file(init_icon);
    net_icon.set_icon_size(gtk4::IconSize::Large);
    let net_label = Label::new(Some(&init_label));
    net_label.add_css_class("netBtnLabel");
    net_label.set_halign(gtk4::Align::Start);
    let net_body = Label::new(Some(&init_body));
    net_body.add_css_class("netBtnBody");
    net_body.set_halign(gtk4::Align::Start);

    let net_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    
    let net_labels_box = GtkBox::new(Orientation::Vertical, 2);
    net_labels_box.append(&net_label);
    net_labels_box.append(&net_body);

    net_box.append(&net_icon);
    net_box.append(&net_labels_box);

    let netbtn = Button::builder()
        .child(&net_box)
        .css_classes(["ctrlBtnL"])
        .build();

    let net_icon_rc  = Rc::new(net_icon);
    let net_label_rc = Rc::new(net_label);
    let net_body_rc  = Rc::new(net_body);

    let net_expanded = Rc::new(RefCell::new(false));

    let wifi_toggle_icon = Image::from_file("/var/lib/cynager/icons/wifi.svg");
    wifi_toggle_icon.set_icon_size(gtk4::IconSize::Large);
    let wifi_toggle_btn = Button::builder()
        .child(&wifi_toggle_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Toggle WiFi adapter")
        .build();

    let refresh_icon = Image::from_file("/var/lib/cynager/icons/refresh.svg");
    refresh_icon.set_icon_size(gtk4::IconSize::Large);
    let refresh_btn = Button::builder()
        .child(&refresh_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Refresh networks")
        .build();

    let net_settings_icon = Image::from_file("/var/lib/cynager/icons/cog.svg");
    net_settings_icon.set_icon_size(gtk4::IconSize::Large);
    let net_settings_btn = Button::builder()
        .child(&net_settings_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Network settings")
        .build();

    let net_panel_actions = GtkBox::new(Orientation::Horizontal, 8);
    net_panel_actions.add_css_class("netPanelActions");
    net_panel_actions.append(&wifi_toggle_btn);
    net_panel_actions.append(&refresh_btn);
    net_panel_actions.append(&net_settings_btn);

    let net_list_box = gtk4::ListBox::new();
    net_list_box.add_css_class("netList");
    net_list_box.set_selection_mode(gtk4::SelectionMode::None);

    let net_list_rc = Rc::new(net_list_box);
    let populate_networks = {
        let net_list_rc = net_list_rc.clone();
        move || {
            while let Some(child) = net_list_rc.first_child() {
                net_list_rc.remove(&child);
            }
            let networks = get_wifi_networks();
            if networks.is_empty() {
                let row = gtk4::Label::new(Some("No networks found"));
                row.add_css_class("netListEmpty");
                net_list_rc.append(&row);
            } else {
                for (ssid, bars, active) in networks {
                    let row_box = GtkBox::new(Orientation::Horizontal, 10);
                    row_box.add_css_class("netListRow");

                    let ssid_lbl = gtk4::Label::new(Some(&ssid));
                    ssid_lbl.set_hexpand(true);
                    ssid_lbl.set_halign(gtk4::Align::Start);
                    ssid_lbl.add_css_class("netListSSID");

                    let signal_lbl = gtk4::Label::new(Some(&bars));
                    signal_lbl.add_css_class("netListSignal");

                    if active {
                        let connected_lbl = gtk4::Label::new(Some("✓"));
                        connected_lbl.add_css_class("netListConnected");
                        row_box.append(&connected_lbl);
                    }
                    row_box.append(&ssid_lbl);
                    row_box.append(&signal_lbl);

                    let row_btn = Button::builder()
                        .child(&row_box)
                        .css_classes(["netListRowBtn"])
                        .build();
                    let ssid_clone = ssid.clone();
                    row_btn.connect_clicked(move |_| {
                        let _ = std::process::Command::new("nmcli")
                            .args(["dev", "wifi", "connect", &ssid_clone])
                            .spawn();
                    });
                    net_list_rc.append(&row_btn);
                }
            }
        }
    };
    let populate_networks_rc = Rc::new(populate_networks);

    let scroll_win = gtk4::ScrolledWindow::new();
    scroll_win.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll_win.set_max_content_height(220);
    scroll_win.set_propagate_natural_height(true);
    scroll_win.set_child(Some(&*net_list_rc));
    scroll_win.add_css_class("netListScroll");

    let net_panel = GtkBox::new(Orientation::Vertical, 6);
    net_panel.add_css_class("netPanel");
    net_panel.append(&net_panel_actions);
    net_panel.append(&scroll_win);
    net_panel.set_visible(false);

    let net_panel_rc = Rc::new(net_panel);

    {
        let wifi_toggle_icon_c = wifi_toggle_icon.clone();
        let adapter_on = Rc::new(RefCell::new(!wifi_soft_blocked()));
        wifi_toggle_btn.connect_clicked(move |_| {
            let currently_on = *adapter_on.borrow();
            toggle_wifi_adapter(!currently_on);
            *adapter_on.borrow_mut() = !currently_on;
            let icon = if !currently_on {
                "/var/lib/cynager/icons/wifi.svg"
            } else {
                "/var/lib/cynager/icons/wifioff.svg"
            };
            wifi_toggle_icon_c.set_from_file(Some(icon));
        });
    }

    {
        let pop = populate_networks_rc.clone();
        refresh_btn.connect_clicked(move |btn| {
            btn.add_css_class("spinning");
            let _ = std::process::Command::new("nmcli")
                .args(["dev", "wifi", "rescan"])
                .spawn();
            let pop = pop.clone();
            let btn_c = btn.clone();
            glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                pop();
                btn_c.remove_css_class("spinning");
            });
        });
    }

    {
        net_settings_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("nm-connection-editor").spawn();
        });
    }

    let net_rx = Rc::new(RefCell::new(net_rx));

    {
        let net_icon_rc  = net_icon_rc.clone();
        let net_label_rc = net_label_rc.clone();
        let net_body_rc  = net_body_rc.clone();
        let net_rx = net_rx.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            let rx = net_rx.borrow();
            let mut latest: Option<NetworkState> = None;
            while let Ok(state) = rx.try_recv() {
                latest = Some(state);
            }
            if let Some(state) = latest {
                let (icon_name, label_text, label_body) = network_icon_and_tip(state);
                net_icon_rc.set_from_file(Some(icon_name));
                net_label_rc.set_label(&label_text);
                net_body_rc.set_label(&label_body);
            }
            glib::ControlFlow::Continue
        });
    }

    let sound_rx = spawn_sound_watcher(Duration::from_secs(3));
    let init_snd = get_sound_state();

    let snd_icon = Image::from_file(sound_icon(&init_snd));
    snd_icon.set_icon_size(gtk4::IconSize::Large);

    let snd_label = Label::new(Some(&format!("{}%", init_snd.volume)));
    snd_label.add_css_class("netBtnLabel");
    snd_label.set_halign(gtk4::Align::Start);

    let snd_body = Label::new(Some(&init_snd.sink));
    snd_body.add_css_class("netBtnBody");
    snd_body.set_halign(gtk4::Align::Start);

    let snd_labels_box = GtkBox::new(Orientation::Vertical, 2);
    snd_labels_box.append(&snd_label);
    snd_labels_box.append(&snd_body);

    let sound_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    sound_box.append(&snd_icon);
    sound_box.append(&snd_labels_box);

    let soundbtn = Button::builder()
        .child(&sound_box)
        .css_classes(["ctrlBtnL"])
        .build();

    let snd_icon_rc = Rc::new(snd_icon);
    let snd_label_rc = Rc::new(snd_label);
    let snd_body_rc = Rc::new(snd_body);
    let sound_rx = Rc::new(RefCell::new(sound_rx));

    {
        let snd_icon_rc = snd_icon_rc.clone();
        let snd_label_rc = snd_label_rc.clone();
        let snd_body_rc = snd_body_rc.clone();
        let sound_rx = sound_rx.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            let rx = sound_rx.borrow();
            let mut latest: Option<SoundState> = None;
            while let Ok(state) = rx.try_recv() {
                latest = Some(state);
            }
            if let Some(state) = latest {
                snd_icon_rc.set_from_file(Some(sound_icon(&state)));
                snd_label_rc.set_label(&format!("{}%", state.volume));
                snd_body_rc.set_label(&state.sink);
            }
            glib::ControlFlow::Continue
        });
    }

    let airplaneicon = Image::from_file("/var/lib/cynager/icons/wifioff.svg");
    airplaneicon.set_icon_size(gtk4::IconSize::Large);

    let airplane: Button = Button::builder()
        .child(&airplaneicon)
        .css_classes(["ctrlBtnS"])
        .tooltip_text("Airplane Mode")
        .build();

    let dndicon = Label::builder()
        .label("DnD.")
        .css_classes(["dndicon"])
        .build();

    let dnd: Button = Button::builder()
        .child(&dndicon)
        .css_classes(["ctrlBtnS"])
        .tooltip_text("Airplane Mode")
        .build();

    let setticon = Image::from_file("/var/lib/cynager/icons/cog.svg");
    setticon.set_icon_size(gtk4::IconSize::Large);

    let setting: Button = Button::builder()
        .child(&setticon)
        .css_classes(["ctrlBtnS"])
        .tooltip_text("Airplane Mode")
        .build();

    let btns = GtkBox::new(Orientation::Horizontal, 16);
    btns.set_css_classes(&["ctrlBTNSbox"]);
    btns.set_halign(gtk4::Align::Center);
    btns.set_valign(gtk4::Align::Start);
    btns.set_margin_top(80);
    btns.set_can_target(true);
    btns.append(&usr);
    btns.append(&netbtn);
    btns.append(&airplane);
    btns.append(&dnd);
    btns.append(&setting);
    btns.append(&soundbtn);
    btns.add_css_class("startingOSD");

    let ctrl_column = GtkBox::new(Orientation::Vertical, 20);
    ctrl_column.set_halign(gtk4::Align::Center);
    ctrl_column.set_valign(gtk4::Align::Start);
    ctrl_column.append(&btns);
    ctrl_column.append(&*net_panel_rc);

    let layout = gtk4::Overlay::new();
    layout.set_child(Some(&backdrop));
    layout.add_overlay(&ctrl_column);
 
    win.set_child(Some(&layout));
 
    let close = {
        let win_c = win.clone();
        let flag = overlay_open.clone();
        Rc::new(move || {
            *flag.borrow_mut() = false;
            win_c.close();
        })
    };

    {
        let close = close.clone();
        backdrop.connect_clicked(move |_| close());
    }
 
    {
        usr.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
        });
    }
 
    {
        let net_panel_rc  = net_panel_rc.clone();
        let net_expanded  = net_expanded.clone();
        let populate      = populate_networks_rc.clone();
        let netbtn_c      = netbtn.clone();
        netbtn.connect_clicked(move |_| {
            let mut expanded = net_expanded.borrow_mut();
            *expanded = !*expanded;
            if *expanded {
                netbtn_c.add_css_class("netBtnExpanded");
                net_panel_rc.set_visible(true);
                populate();
            } else {
                netbtn_c.remove_css_class("netBtnExpanded");
                net_panel_rc.set_visible(false);
            }
        });
    }

    {
        let airplaneicon_c = airplaneicon.clone();
        airplane.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            airplaneicon_c.add_css_class("flyplane");
            let airplaneicon_timeout = airplaneicon_c.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
                airplaneicon_timeout.remove_css_class("flyplane");
                glib::ControlFlow::Break 
            });
        });
    }

    {
        setting.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            // close();
        });
    }

    {
        soundbtn.connect_clicked(move |_| {
            // let _ = std::process::Command::new("pavucontrol").spawn();
            close();
        });
    }

    {
        let snd_icon_rc  = snd_icon_rc.clone();
        let snd_label_rc = snd_label_rc.clone();
        let snd_body_rc  = snd_body_rc.clone();

        let scroll = EventControllerScroll::new(
            EventControllerScrollFlags::VERTICAL | EventControllerScrollFlags::DISCRETE,
        );

        scroll.connect_scroll(move |_, _dx, dy| {
            let step = if dy < 0.0 { "5%+" } else { "5%-" };
            let _ = std::process::Command::new("wpctl")
                .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", step])
                .spawn();

            glib::timeout_add_local_once(Duration::from_millis(80), {
                let snd_icon_rc  = snd_icon_rc.clone();
                let snd_label_rc = snd_label_rc.clone();
                let snd_body_rc  = snd_body_rc.clone();
                move || {
                    let state = get_sound_state();
                    snd_icon_rc.set_from_file(Some(sound_icon(&state)));
                    snd_label_rc.set_label(&format!("{}%", state.volume));
                    snd_body_rc.set_label(&state.sink);
                }
            });

            glib::Propagation::Stop
        });

        soundbtn.add_controller(scroll);
    }
 
    win.present();
}