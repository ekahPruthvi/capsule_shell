use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, Button, Image, Revealer, LevelBar
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use chrono::Local;
use std::cell::RefCell;
use std::cell::Cell;
use std::rc::Rc;
use std::fs;
use std::process::{Command, Stdio};
use std::time::UNIX_EPOCH;
use std::path::Path;
use std::io::{BufReader, BufRead};
use glib::{timeout_add_seconds_local, ControlFlow::{Continue, Break}};
use std::thread;


pub fn start_status_icon_updater(container: &Rc<GtkBox>) {
    // Initial population
    append_status_icons(container);

    // Set interval to refresh every 10 seconds
    let container_clone = container.clone();
    glib::timeout_add_seconds_local(5, move || {
        // Clear existing icons
        while let Some(child) = container_clone.first_child() {
            container_clone.remove(&child);
        }

        // Re-add updated battery + network icons
        append_status_icons(&container_clone);

        Continue // keep repeating
    });
}

fn get_battery_info() -> Option<(u8, String)> {
    for bat in &["BAT1", "BAT0", "BAT2"] {
        let base = format!("/sys/class/power_supply/{}", bat);
        let cap_path = format!("{}/capacity", base);
        let stat_path = format!("{}/status", base);

        if Path::new(&cap_path).exists() && Path::new(&stat_path).exists() {
            let cap = fs::read_to_string(&cap_path).ok()?.trim().parse::<u8>().ok()?;
            let status = fs::read_to_string(&stat_path).ok()?.trim().to_string();
            return Some((cap, status));
        }
    }
    None
}

fn get_network_info() -> String {

    let space = r#"
        #!/bin/bash
        wire=$(nmcli device status | grep Wired)
        if [ -n "$wire" ]; then
            printf "Wired"
        else
            state=$(nmcli -fields WIFI g)
            test=$(echo "$state" | grep "enabled")
            if [ $test = "enabled" ]; then
                net=$(nmcli -t -f active,SSID device wifi | awk -F: '/^yes:/ {print $2}')
                if [[ -z $net ]]; then
                    printf "No Connection"
                else
                    printf "Network Enabled - $net"
                fi
            else
                printf "Network Disabled"
            fi
        fi
    "#;

    let command = &space;
    let output = Command::new("sh")
        .args(["-c", command])
        .output()
        .unwrap_or_else(|_| panic!("Failed to run nmcli"));

    let stdout = String::from_utf8_lossy(&output.stdout);

    return stdout.to_string();
}


fn create_icon_with_tooltip(icon_name: &str, tooltip: &str) -> Button {
    let image = Image::from_icon_name(icon_name);
    image.set_icon_size(gtk4::IconSize::Normal);

    let button = Button::builder()
        .child(&image)
        .tooltip_text(tooltip)
        .build();

    button.set_widget_name("statusicon");
    button.set_css_classes(&["statusicon"]);
    
    button
}


pub fn append_status_icons(container: &GtkBox) {
    // Network
    let network_info = get_network_info();
    let net_icon = if network_info.contains("Wired") {
        "network-wired-symbolic"
    } else if network_info.contains("Network Enabled") {
        "network-wireless-signal-excellent-symbolic"
    } else if network_info.contains("No Connection") {
        "network-wireless-offline-symbolic"
    } else {
        "network-offline-symbolic"
    };

    let network_btn = create_icon_with_tooltip(net_icon, &network_info);
    container.append(&network_btn);
    
    // Battery
    if let Some((percent, status)) = get_battery_info() {
        let icon_name = if status == "Charging" {
            "sensors-voltage-symbolic"
        } else if percent > 80 {
            "battery-full-symbolic"
        } else if percent > 60 {
            "battery-good-symbolic"
        } else if percent > 40 {
            "battery-medium-symbolic"
        } else if percent > 20 {
            "battery-low-symbolic"
        } else {
            "battery-caution-symbolic"
        };

        let tooltip = format!("Battery: {}% ({})", percent, status);
        let battery_btn = create_icon_with_tooltip(icon_name, &tooltip);
        container.append(&battery_btn);
    }

    
}


fn create_icon_button(icon_name: &str, exec_command: String) -> Button {
    // You can also use Image::from_icon_name if it's a known system icon
    let image = Image::from_icon_name(icon_name);
    image.set_icon_size(gtk4::IconSize::Normal);

    let button = Button::builder()
        .child(&image)
        .tooltip_text(&exec_command)
        .build();

    button.connect_clicked(move |_| {
        let _ = Command::new("sh")
            .arg("-c")
            .arg(&exec_command)
            .spawn();
    });

    button
}

fn ql_creator(container: &GtkBox, commands: Rc<RefCell<Vec<String>>>, last_hash: Rc<RefCell<u64>>) {
     
    let home = std::env::var("HOME").unwrap_or_default();
    let qlpath = format!("{}/.config/alt/ql.dat", home);

    if let Ok(metadata) = fs::metadata(&qlpath) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let new_hash = duration.as_secs();
                let mut last = last_hash.borrow_mut();
                if *last == new_hash {
                    return; // No change
                }
                *last = new_hash;
            }
        }
    }

    while let Some(child) = container.first_child() {
        container.remove(&child);
    }  

    commands.borrow_mut().clear();

    if let Ok(contents) = fs::read_to_string(&qlpath) {
        let mut exec = None;
        let mut icon = None;

        for line in contents.lines() {
            if line.starts_with("Exec=") {
                exec = Some(line.trim_start_matches("Exec=").to_string());
            } else if line.starts_with("Icon=") {
                icon = Some(line.trim_start_matches("Icon=").to_string());
            }

            if let (Some(exec_val), Some(icon_val)) = (&exec, &icon) {
                let exec_clone = exec_val.clone();
                commands.borrow_mut().push(exec_clone.clone());

                let button = create_icon_button(&icon_val, exec_clone);
                button.set_margin_bottom(5);
                button.set_widget_name("qlicons");
                button.set_css_classes(&["qlicons"]);
                container.append(&button);

                exec = None;
                icon = None;
            }
        }
    }
}

fn check(container: &Rc<GtkBox>, prev: &Rc<RefCell<String>>){

    let app = r#"
        tac /tmp/notiv.dat | grep -m1 "appname:" | sed "s/.*appname: *'//; s/'.*//"
    "#;

    let command = &app;
    let app_output = Command::new("sh")
        .args(["-c", command])
        .output()
        .unwrap_or_else(|_| panic!("Failed to check notiv.dat"));
    let app_stdout = String::from_utf8_lossy(&app_output.stdout);

    let notification = r#"
        #!/bin/bash
        awk '
        BEGIN { RS="}\n"; FS="\n" }
        /formatted:/ { last=$0 }
        END {
            match(last, /formatted: *'\''([^'\'']*)'\'',?/, m)
            if (m[1] != "") print m[1]
        }
        ' /tmp/notiv.dat
    "#;

    let command = &notification;
    let notification_output = Command::new("sh")
        .args(["-c", command])
        .output()
        .unwrap_or_else(|_| panic!("Failed to check notiv.dat"));
    let notification_stdout = String::from_utf8_lossy(&notification_output.stdout);

    if *prev.borrow() == notification_stdout {
        return;
    }

    *prev.borrow_mut() = notification_stdout.to_string();

    let notification_label = Label::new(None);
    notification_label.set_markup(&format!("{}{}", notification_stdout, app_stdout));
    notification_label.set_widget_name("notivlabel");
    notification_label.set_vexpand(true);
    notification_label.set_valign(gtk4::Align::Center); 
    notification_label.set_justify(gtk4::Justification::Left);
    

    let mut width = 0;
    container.set_height_request(49);
    let container_clone = container.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(2), move || {
        if width < 200 {
            width += 1;
            container_clone.set_width_request(width);
            Continue
        } else {
            container_clone.append(&notification_label);
            Break
        }
    });
    

}

pub fn notiv_maker(container: &Rc<GtkBox>) {
    // Initial population
    let prev_noti = Rc::new(RefCell::new(String::new()));
    check(container, &prev_noti);

    // Set interval to refresh every 5 seconds
    let container_clone = container.clone();
    glib::timeout_add_seconds_local(5, move || {
        // Clear existing notification
        let mut _child_removed = false;
        while let Some(child) = container_clone.first_child() {
            container_clone.remove(&child);   
            _child_removed = true;
        }

        if _child_removed {
            let mut width = 200;
            let container_clone_final = container_clone.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(2), move || {
                if width > 0 {
                    width -= 1;
                    container_clone_final.set_width_request(width);
                    Continue
                } else {
                    _child_removed = false;
                    Break
                }
            });
        }
        

        // Re-add updated notifications
        check(&container_clone, &prev_noti);

        Continue // keep repeating
    });
}


fn activate(app: &Application) {
    
    // Main full-screen dashboard ----------------------------------------------------------------------------------------------------------------------------- //
    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Bottom);
    window.auto_exclusive_zone_enable();
    window.fullscreen();
    window.set_decorated(false);
    window.set_namespace(Some("capsule"));

    for (edge, anchor) in [
        (Edge::Left, true),
        (Edge::Right, true),
        (Edge::Top, true),
        (Edge::Bottom, true),
    ] {
        window.set_anchor(edge, anchor);
    }


    let css = CssProvider::new();
    css.load_from_data(
        "
        label.time {
            font-size: 16px;
            font-weight: 900;
            color:rgba(255, 255, 255, 0.83);
        }

        label.date {
            font-size: 12px;
            color:rgba(255, 255, 255, 0.75);
            font-weight: 600;
        }

        window {
            background-color: rgba(20, 20, 20, 0);
        }

        #timecapsule {
            background-color: rgba(0, 0, 0, 0.2);
            border-radius: 50px;
            padding-left: 20px;
            padding-right: 20px;
            padding-top: 5px;
            padding-bottom: 5px;
            border: 0.5px solid rgba(255, 255, 255, 0.12);
        }
        
        #qlbar {
            background-color: rgba(0, 0, 0, 0.2);
            border-radius: 50px;
            padding: 5px;
            border: 0.5px solid rgba(255, 255, 255, 0.12);
        }

        button.qlicons {
            all: unset;
            border-radius: 50px;
            padding: 10px;
            background-color: rgba(49, 49, 49, 0);
            transition: background-color 0.2s ease, transform 0.2s ease;
            transform: scale(1.0);
        }

        button.qlicons:hover {
            background-color: rgb(29, 29, 29);
            border-radius: 10px;
            transform: scale(1.5);
        }

        button.cos {
            all: unset;
            border-radius: 50px;
            padding: 10px;
            background-color: rgba(49, 49, 49, 0);
            transition: transform 0.2s ease;
            transform: scale(1.0);
        }

        button.cos:hover {
            transform: scale(1.1);
        }

        button.statusicon {
            all: unset;
            padding: 10px;
            background-color: rgba(49, 49, 49, 0);
            transition: color 0.2s ease;
            color: rgb(197, 197, 197);
        }

        button.statusicon:hover {
            color: rgb(255, 255, 255);
        }

        #cynbar {
            background-color: rgba(0, 0, 0, 0.12);
            border-radius: 50px;
            padding-left: 5px;
            padding-right: 5px;
            padding-top: 5px;
            padding-bottom: 10px;
            border: 0.5px solid rgba(255, 255, 255, 0.12);
        }

        #noticapsule {
            background-color: rgba(0, 0, 0, 0.12);
            border-radius: 50px;
            padding: 5px;
            border: 0.5px solid rgba(255, 255, 255, 0.12);
        }
        
        #notivlabel {
            font-size: 12px;
            font-weight: 300;
            color:rgba(255, 255, 255, 0.83);
        }

        #notivbox {
            padding-top: 10px;
        }

        #volumelevel.muted {
            background-color: crimson;
            opacity: 0.6;
        }

    ",
    );

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // for quicklaunch icons ----------------------------------------------------------------------------------------------------------------------------- //
    let qlbox = GtkBox::new(Orientation::Vertical, 0);
    qlbox.set_widget_name("qlbar");

    let commands = Rc::new(RefCell::new(Vec::new()));
    let last_hash = Rc::new(RefCell::new(0u64));

    // for quicklauncher connected to alt. 
    ql_creator(&qlbox, commands.clone(), last_hash.clone());

    // check every 1s to check for updates
    {
        let boxxy_clone = qlbox.clone();
        timeout_add_seconds_local(1, move || {
            ql_creator(&boxxy_clone, commands.clone(), last_hash.clone());
            glib::ControlFlow::Continue
        });
    }

    // top bar ----------------------------------------------------------------------------------------------------------------------------- //
    let noticapsule = GtkBox::new(Orientation::Horizontal, 10);
    noticapsule.set_widget_name("noticapsule");
    noticapsule.set_halign(gtk4::Align::Center);
    noticapsule.set_valign(gtk4::Align::Start);
    noticapsule.set_margin_end(69);
    noticapsule.set_margin_top(10);

    // time capsule ----------------------------------------------------------------------------------------------------------------------------- //
    let timedatebox = GtkBox::new(Orientation::Horizontal, 30);
    timedatebox.set_halign(gtk4::Align::Center);

    let time_label = Label::new(None);
    time_label.set_css_classes(&["time"]);

    let date_label = Label::new(None);
    date_label.set_css_classes(&["date"]);

    let now = Local::now();
    time_label.set_text(&now.format("%I %M %p").to_string());
    date_label.set_text(&now.format("%d %B %Y\n%A").to_string());

    let time_label_ref = Rc::new(RefCell::new(time_label));
    timeout_add_seconds_local(1, {
        let time_label = time_label_ref.clone();
        move || {
            let now = Local::now();
            time_label.borrow().set_text(&now.format("%I %M %p").to_string());
            Continue
        }
    });

    timedatebox.append(&*time_label_ref.borrow());
    timedatebox.append(&date_label);
    timedatebox.set_widget_name("timecapsule");

    // notifications ----------------------------------------------------------------------------------------------------------------------------- //
    let notiv_box = Rc::new(GtkBox::new(gtk4::Orientation::Horizontal, 0));
    notiv_box.set_widget_name("notivbox");
    notiv_box.set_halign(gtk4::Align::Center);


    notiv_maker(&notiv_box);

    // cos logo only works with cynide iconpack -------------------------------------------------------------------------------------------------- //
    let cos=create_icon_button("cos", "mpv --no-video ~/.config/hypr/startup.mp3".to_string());
    if let Some(child) = cos.child() {
        if let Some(image) = child.downcast_ref::<Image>() {
            image.set_icon_size(gtk4::IconSize::Large);
        }
    }
    cos.set_tooltip_text(Some("CynageOS"));
    cos.set_css_classes(&["cos"]);

    // OSD volume ------------------------------------------------------------------------------------------------------------------------------- //
    let osd_box = Rc::new(GtkBox::new(Orientation::Horizontal, 6));
    osd_box.set_widget_name("osdbox");
    osd_box.set_halign(gtk4::Align::Center);
    osd_box.set_valign(gtk4::Align::Center);
    osd_box.set_margin_top(10);
    osd_box.set_margin_bottom(10);
    osd_box.set_visible(false); // hidden initially

    // Create the LevelBar
    let level_bar = Rc::new(LevelBar::new());
    level_bar.set_widget_name("volumelevel");
    level_bar.set_min_value(0.0);
    level_bar.set_max_value(100.0);
    level_bar.set_value(0.0);
    level_bar.set_size_request(200, 20);
    level_bar.set_sensitive(false);

    osd_box.append(&*level_bar);
    noticapsule.append(&cos);
    noticapsule.append(&*osd_box);

    // Spawn a background thread to listen for volume changes
    let (tx, rx) = async_channel::unbounded::<(f64, bool)>();

    // Thread: subscribe to pactl and send volumes
    thread::spawn(move || {
        let mut child = Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to run pactl");

        let mut last_vol: Option<u8> = None;
        let mut last_mute: Option<bool> = None;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if line.contains("Event 'change' on sink") {
                    // Volume
                    let vol_output = Command::new("pactl")
                        .args(&["get-sink-volume", "@DEFAULT_SINK@"])
                        .output()
                        .ok()
                        .and_then(|out| String::from_utf8(out.stdout).ok());

                    let new_vol = vol_output
                        .as_ref()
                        .and_then(|txt| txt.split('/').nth(1))
                        .and_then(|v| v.trim().trim_end_matches('%').parse::<u8>().ok())
                        .unwrap_or(0);

                    // Mute
                    let mute_output = Command::new("pactl")
                        .args(&["get-sink-mute", "@DEFAULT_SINK@"])
                        .output()
                        .ok()
                        .and_then(|out| String::from_utf8(out.stdout).ok());

                    let new_mute = mute_output
                        .map(|txt| txt.contains("yes"))
                        .unwrap_or(false);

                    // Skip if nothing changed
                    if Some(new_vol) == last_vol && Some(new_mute) == last_mute {
                        continue;
                    }

                    last_vol = Some(new_vol);
                    last_mute = Some(new_mute);

                    let _ = tx.send_blocking((new_vol as f64, new_mute));
                }
            }
        }
    });

    // to autohide the slider
    let hide_timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let hide_timeout_id_clone = hide_timeout_id.clone();
    let osd_box_clone = osd_box.clone();

    // Main thread: receive and update UI
    glib::MainContext::default().spawn_local(async move {
        while let Ok((vol, is_muted)) = rx.recv().await {
            // update level_bar as before
            if is_muted {
                level_bar.set_value(0.0);
                level_bar.add_css_class("muted");
            } else {
                level_bar.set_value(vol);
                level_bar.remove_css_class("muted");
            }

            osd_box.set_visible(true);

            // cancel previous hide timeout if any
            if let Some(id) = hide_timeout_id_clone.borrow_mut().take() {
                id.remove();
            }

            // set new hide timeout
            let osd_box_clone_inner = osd_box_clone.clone();
            let id = glib::timeout_add_seconds_local(2, move || {
                osd_box_clone_inner.set_visible(false);
                Continue
            });

            *hide_timeout_id_clone.borrow_mut() = Some(id);
        }
    });


    noticapsule.append(&*notiv_box);
    noticapsule.append(&timedatebox);

    // verticcal bar revealer --------------------------------------------------------------------------------------------------------------------------------- //
    let revealer = Revealer::builder()
            .transition_type(gtk4::RevealerTransitionType::Crossfade)
            .transition_duration(500)
            .reveal_child(false)
            .build();
    
    let hover = Rc::new(Cell::new(false));
    let hover_clone = hover.clone();
    let revealer_clone = revealer.clone();

    let motion_controller = gtk4::EventControllerMotion::new();
    motion_controller.connect_enter(move |_, _x, _y| {
        hover_clone.set(true);
        revealer_clone.set_reveal_child(true);
    });

    let revealer_clone2 = revealer.clone();
    let hover_clone2 = hover.clone();
    motion_controller.connect_leave(move |_| {
        hover_clone2.set(false);
        revealer_clone2.set_reveal_child(false);
    });

    window.add_controller(motion_controller);

    // vertical bar ----------------------------------------------------------------------------------------------------------------------------- //
    let boxxy = GtkBox::new(Orientation::Vertical, 2);
    boxxy.set_valign(gtk4::Align::Center);
    boxxy.set_halign(gtk4::Align::End);
    boxxy.set_margin_start(10);
    boxxy.set_widget_name("cynbar");

    let status_box = Rc::new(GtkBox::new(gtk4::Orientation::Vertical, 5));
    start_status_icon_updater(&status_box);

    let power=create_icon_button("system-shutdown-symbolic", "terminatee".to_string());
    power.set_tooltip_text(Some("Power menu"));
    power.set_css_classes(&["statusicon"]);
    
    boxxy.append(&qlbox);
    boxxy.append(&power);
    boxxy.append(&*status_box);


    revealer.set_child(Some(&boxxy));
    
    
    // main box window ----------------------------------------------------------------------------------------------------------------------------- //
    let hdummy_start = GtkBox::new(Orientation::Horizontal, 0);
    hdummy_start.set_hexpand(true);

    let hdummy_end = GtkBox::new(Orientation::Horizontal, 0);
    hdummy_end.set_hexpand(true);

    let dbox = GtkBox::new(Orientation::Horizontal, 0);
    // dbox.set_valign(gtk4::Align::Center);

    dbox.append(&revealer);
    dbox.append(&hdummy_start);
    dbox.append(&noticapsule);
    dbox.append(&hdummy_end);

    window.set_child(Some(&dbox));

    window.show();
}


fn main() {
    // Start reading from here dumbass

    let app = Application::new(Some("com.ekah.cynideshell"), Default::default());
    app.connect_activate(activate);

    app.run();

}