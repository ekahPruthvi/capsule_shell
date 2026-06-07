use gtk4::{
    Application, ApplicationWindow, Label, Box as GtkBox, Button, Orientation, prelude::*,
    DrawingArea, gdk_pixbuf::Pixbuf, Image,
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
    win.set_namespace(Some("CtrlOerlay"));
    win.set_layer(Layer::Overlay);
    win.remove_css_class("background");
    win.set_anchor(Edge::Top,    true);
    win.set_anchor(Edge::Bottom, true);
    win.set_anchor(Edge::Left,   true);
    win.set_anchor(Edge::Right,  true);
    win.set_exclusive_zone(-1);
 
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
    let size = 30;

    let usricon = DrawingArea::new();
    usricon.set_content_width(size);
    usricon.set_content_height(size);

    usricon.set_draw_func(move |_, cr, w, h| {
        let w = w as f64;
        let h = h as f64;

        cr.arc(w / 2.0, h / 2.0, w / 2.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.clip();

        let pb = pixbuf.scale_simple(w as i32, h as i32, gtk4::gdk_pixbuf::InterpType::Bilinear).unwrap();
        cr.set_source_pixbuf(&pb, 0.0, 0.0);
        cr.paint().unwrap();
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

    usrbox.append(&Label::builder()
        .label(&format!("Hello,{}.", name))
        .css_classes(["username"])
        .build()
    );
    
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
    let net_body_rc = Rc::new(net_body);

    let net_rx = Rc::new(RefCell::new(net_rx));

    {
        let net_icon_rc  = net_icon_rc.clone();
        let net_label_rc = net_label_rc.clone();
        let net_body_rc = net_body_rc.clone();
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

    let airplaneicon = Image::from_file("/var/lib/cynager/icons/wifioff.svg");
    airplaneicon.set_icon_size(gtk4::IconSize::Large);

    let airplane: Button = Button::builder()
        .child(&airplaneicon)
        .css_classes(["ctrlBtnS"])
        .build();

    let btns = GtkBox::new(Orientation::Horizontal, 16);
    btns.set_halign(gtk4::Align::Center);
    btns.set_valign(gtk4::Align::Start);
    btns.set_margin_top(100);
    btns.set_can_target(true);
    btns.append(&usr);
    btns.append(&netbtn);
    btns.append(&airplane);
    btns.add_css_class("starting");

    let layout = gtk4::Overlay::new();
    layout.set_child(Some(&backdrop));
    layout.add_overlay(&btns);
 
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
        let close = close.clone();
        usr.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            close();
        });
    }
 
    {
        let close = close.clone();
        netbtn.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            close();
        });
    }

    {
        let close = close.clone();
        airplane.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            close();
        });
    }
 
    win.present();
}