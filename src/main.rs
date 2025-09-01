pub mod notification_extd;
pub mod desktoppy;
use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, Button, Image, LevelBar, EventControllerScroll, 
    EventControllerScrollFlags
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use chrono::Local;
use std::cell::RefCell;
use std::rc::Rc;
use std::fs;
use std::process::{Command, Stdio};
use std::time::{UNIX_EPOCH, Duration};
use std::path::Path;
use std::env;
use std::io::{BufReader, BufRead};
use glib::{timeout_add_seconds_local, ControlFlow::{Continue, Break}, idle_add_local};
use std::thread;
use inotify::{Inotify, WatchMask};
use rand::rng;
use rand::prelude::IndexedRandom;
use gtk4::gio::File;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use signal_hook::consts::signal::*;
use signal_hook::flag;


pub fn start_status_icon_updater(container: &Rc<GtkBox>) {
    append_status_icons(container);

    let container_clone = container.clone();
    glib::timeout_add_seconds_local(5, move || {
        while let Some(child) = container_clone.first_child() {
            container_clone.remove(&child);
        }

        append_status_icons(&container_clone);

        Continue
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
    
    let qlpath = format!("/var/lib/cynager/ql.dat");

    if let Ok(metadata) = fs::metadata(&qlpath) {
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let new_hash = duration.as_secs();
                let mut last = last_hash.borrow_mut();
                if *last == new_hash {
                    return;
                }
                *last = new_hash;
            }
        }
    }

    while let Some(child) = container.first_child() {
        container.remove(&child);
    }  

    commands.borrow_mut().clear();

    let mut noneicon = true;

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
                noneicon = false;
                exec = None;
                icon = None;
            }
        }
    }

    if noneicon {
        let button = create_icon_button("transperent","altDot".to_string() );
        button.set_margin_bottom(5);
        button.set_widget_name("qlicons");
        button.set_css_classes(&["qlicons"]);
        container.append(&button);
    }
}


fn check(container: &Rc<GtkBox>, prev: &Rc<RefCell<String>>, notiwidth: &Rc<RefCell<usize>>, apppy: &Application, timedatebox: &GtkBox) {

    let app = r#"
        tr -d '\000' < /tmp/notiv.dat | tac | grep -m1 "appname:" | sed "s/.*appname: *'//; s/'.*//"
    "#;

    let command = &app;
    let app_output = Command::new("sh")
        .args(["-c", command])
        .output()
        .unwrap_or_else(|_| panic!("Failed to check notiv.dat"));
    let app_stdout = String::from_utf8_lossy(&app_output.stdout);

    let check_actions = r#"
        tac /tmp/notiv.dat | awk '
        BEGIN { RS="}\n"; found=0 }
        !found && /appname:/ {
            block = $0 "}"
            found = 1
        }
        END {
            if (block ~ /actions: *\{/ && block ~ /"[a-zA-Z0-9_]+":/) print "yes"
        }'
    "#;

    let actions_output = Command::new("sh")
        .args(["-c", check_actions])
        .output()
        .unwrap_or_else(|_| panic!("Failed to check for actions"));
    let actions_present = String::from_utf8_lossy(&actions_output.stdout).trim() == "yes";
    
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

    let cont_time_clone = timedatebox.clone();
    if actions_present {
        cont_time_clone.remove_css_class("scale-in");
        while let Some(child) = cont_time_clone.first_child() {
            cont_time_clone.remove(&child);
        }
        let actions_btn = Button::builder().child(&Label::new(Some("Actions"))).build();
        actions_btn.set_css_classes(&["notification_btn"]);
        actions_btn.connect_clicked(move |_| {
            thread::spawn(move || {
                let _ = Command::new("dunstctl")
                    .arg("context")
                    .output();

                thread::sleep(Duration::from_secs(40));

                let _ = Command::new("dunstctl")
                    .arg("close-all")
                    .output();
            });
        });
        cont_time_clone.append(&actions_btn);
            idle_add_local(move || {
            cont_time_clone.add_css_class("scale-in");
            Break
        });
    }

    let notification_label = Label::new(None);
    notification_label.set_markup(&format!("{}{}", notification_stdout, app_stdout));
    notification_label.set_wrap(true);
    notification_label.set_ellipsize(gtk4::pango::EllipsizeMode::End); 
    notification_label.set_max_width_chars(170);
    notification_label.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    notification_label.set_widget_name("notivlabel");
    notification_label.set_vexpand(true);
    notification_label.set_valign(gtk4::Align::Center); 
    notification_label.set_justify(gtk4::Justification::Left);

    let app_clone = apppy.clone();
    let noti_button = Button::builder().child(&notification_label).build();
    noti_button.set_css_classes(&["notification_btn"]);
    noti_button.connect_clicked(move |_| {
        notification_extd::build_window(&app_clone);
    });
    
    let (_min_width, nat_width, _min_baseline, _nat_baseline) = notification_label.measure(Orientation::Horizontal, -1);
    *notiwidth.borrow_mut() = nat_width as usize;

    let mut width = 0;
    container.set_height_request(49);
    let container_clone = container.clone();
    let notiwidth_clone = notiwidth.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(2), move || {
        if width < *notiwidth_clone.borrow() {
            width += 1;
            container_clone.set_width_request(width as i32);
            Continue
        } else {
            container_clone.append(&noti_button);
            let home = std::env::var("HOME").unwrap();
            let path = format!("{}/.config/hypr/sound/notiv/notiv.mp3", home);
            if let Err(err) = Command::new("mpv")
                .args(["--no-video", "--volume=60", &path])
                .spawn() 
            {
                eprintln!("Failed to play sound: {}", err);
            }
            Break
        }
    });
    

}


pub fn notiv_maker(container: &Rc<GtkBox>, app: &Application, timedatebox: &GtkBox) {
    let notivwidth = Rc::new(RefCell::new(200));
    let prev_noti = Rc::new(RefCell::new(String::new()));
    check(container, &prev_noti, &notivwidth, app, timedatebox);

    let container_clone = container.clone();
    let app_clone = app.clone();
    let time_box_clone = timedatebox.clone();
    glib::timeout_add_seconds_local(3, move || {
        let mut _child_removed = false;
        while let Some(child) = container_clone.first_child() {
            let cont_time_clone = time_box_clone.clone();
            container_clone.remove(&child);
            append_time_and_date_labels(&cont_time_clone, &app_clone);
            _child_removed = true;
        }

        if _child_removed {
            let mut width = *notivwidth.borrow() as i32;
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
        
        check(&container_clone, &prev_noti, &notivwidth, &app_clone, &time_box_clone);

        Continue
    });
}

fn append_time_and_date_labels(timedatebox: &GtkBox, app: &Application) {
    let container_clone = timedatebox.clone();
    timedatebox.remove_css_class("scale-in");
    while let Some(child) = container_clone.first_child() {
        container_clone.remove(&child);
    }

    let time_label = Label::new(None);
    time_label.add_css_class("time");

    let date_label = Label::new(None);
    date_label.add_css_class("date");

    let now = Local::now();
    time_label.set_text(&now.format("%I:%M %p").to_string());
    date_label.set_text(&now.format("%A\n%d %B %Y").to_string());

    let time_label_ref = Rc::new(RefCell::new(time_label));
    let time_date_inner_box = GtkBox::new(Orientation::Horizontal, 30);
    time_date_inner_box.append(&*time_label_ref.borrow());
    time_date_inner_box.append(&date_label);
    let time_date_button = Button::builder().child(&time_date_inner_box).css_classes(["notification_btn"]).build();
    let app_clone = app.clone();
    time_date_button.connect_clicked(move |_| {
        notification_extd::build_window(&app_clone);
    });
    
    timedatebox.append(&time_date_button);

    let time_clone = timedatebox.clone();
    idle_add_local(move || {
        time_clone.add_css_class("scale-in");
        Break
    });

    timeout_add_seconds_local(1, {
        let time_label = time_label_ref.clone();
        move || {
            let now = Local::now();
            time_label.borrow().set_text(&now.format("%I:%M %p").to_string());
            Continue
        }
    });
}


fn activate(app: &Application) {
    
    // css auto reload 

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
            eprintln!("Reloading CSS...");
            css.load_from_file(&file);
        }
        Continue
    });

    // for quicklaunch icons ----------------------------------------------------------------------------------------------------------------------------- //
    let qlbox = GtkBox::new(Orientation::Vertical, 0);
    qlbox.set_css_classes(&["qlbar"]);
    qlbox.set_margin_top(2);
    qlbox.set_margin_start(2);
    qlbox.set_margin_end(2);

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
    noticapsule.set_margin_top(10);

    // time capsule ----------------------------------------------------------------------------------------------------------------------------- //
    let timedatebox = GtkBox::new(Orientation::Horizontal, 0);
    timedatebox.set_halign(gtk4::Align::Center);
    timedatebox.set_margin_top(2);
    timedatebox.set_margin_bottom(2);
    timedatebox.set_margin_end(2);

    append_time_and_date_labels(&timedatebox, &app);
    timedatebox.set_widget_name("timecapsule");

    // notifications ----------------------------------------------------------------------------------------------------------------------------- //
    let notiv_box = Rc::new(GtkBox::new(gtk4::Orientation::Horizontal, 0));
    notiv_box.set_widget_name("notivbox");
    notiv_box.set_halign(gtk4::Align::Center);

    notiv_maker(&notiv_box, app, &timedatebox);

    // cos logo only works with cynide iconpack -------------------------------------------------------------------------------------------------- //
    let cos = Button::new();
    let cos_logo = Image::from_icon_name("cos");
    cos_logo.set_icon_size(gtk4::IconSize::Large);
    cos.set_child(Some(&cos_logo));

    cos.set_tooltip_text(Some("CynageOS"));
    cos.set_css_classes(&["cos"]);

    let messages = vec![
        ("hello", "im cynide from cynageOS"),
        ("Tips:", "use cynidectl to dig deeper"),
        ("alert", "system is stable"),
        ("Oh do you know Bob the builder ?", "by anyy chance ???"),
        ("Try pressing the wallpaper tab x times", " 2x + 5 = 15 "),
        ("'penglins'", "not a typo"),
        ("I'm a repetitive task, you see,My steps are the same, a sequence to be.The number of times,", "a personal clue is the day you arrived"),
        ("make a cock a doodle do", "try super + g"),
        ("cp77", "is sooo coool"),
        ("Hint:", "Settings"),
        ("CYNAGEOSSS", "is thaaa besstttt ?"),
        ("Fun Fact", "capsule is written in rust"),
        ("A strand on the beach ?", "Thats horrific")
    ];

    let messages_clone = messages.clone();
    cos.connect_clicked(move |_| {
        let mut rng = rng();
        if let Some((title, body)) = messages_clone.choose(&mut rng) {
            let _ = Command::new("notify-send")
                .arg("--app-name=cynide")
                .arg(title)
                .arg(body)
                .spawn();
        }
    });

    // OSD -------------------------------------------------------------------------------------------------------------------------------------- //

    #[derive(Debug)]
    enum OsdEvent {
        Volume { level: f64, muted: bool },
        Brightness { level: f64 },
        MicMute { muted: bool },
    }

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
    let (tx, rx) = async_channel::unbounded::<OsdEvent>();

    // Thread: subscribe to pactl and send volumes
    let tx_volume = tx.clone();
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

                    let _ = tx_volume.send_blocking(OsdEvent::Volume { level: new_vol as f64, muted: new_mute });
                }
            }
        }
    });
    
    // thread for brightness
    let tx_brightness = tx.clone();
    thread::spawn(move || {
        let backlight_dir = "/sys/class/backlight";
        let entries = fs::read_dir(backlight_dir).expect("No backlight device found");
        let device = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.join("brightness").exists() && p.join("max_brightness").exists())
            .expect("No valid backlight device found");

        let brightness_path = device.join("brightness");
        let max_brightness_path = device.join("max_brightness");

        let max_brightness = fs::read_to_string(&max_brightness_path)
            .expect("Failed to read max_brightness")
            .trim()
            .parse::<u32>()
            .expect("Invalid max_brightness");

        let mut inotify = Inotify::init().expect("Failed to init inotify");
        inotify
            .watches()
            .add(&brightness_path, WatchMask::MODIFY)
            .expect("Failed to add watch");

        let mut buffer = [0; 1024];

        loop {
            let events = inotify.read_events_blocking(&mut buffer).expect("Failed to read inotify events");

            for _ in events {
                let val = fs::read_to_string(&brightness_path)
                    .ok()
                    .and_then(|s| s.trim().parse::<u32>().ok())
                    .unwrap_or(0);

                let percent = (val as f64 / max_brightness as f64) * 100.0;
                let _ = tx_brightness.send_blocking(OsdEvent::Brightness { level: percent });
            }
        }
    });

    //thread for mic
    let tx_mic = tx.clone();
    thread::spawn(move || {
        let mut child = Command::new("pactl")
            .arg("subscribe")
            .stdout(Stdio::piped())
            .spawn()
            .expect("Failed to run pactl for mic");

        let mut last_mute: Option<bool> = None;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines().flatten() {
                if line.contains("Event 'change' on source") {
                    let mute_output = Command::new("pactl")
                        .args(&["get-source-mute", "@DEFAULT_SOURCE@"])
                        .output()
                        .ok()
                        .and_then(|out| String::from_utf8(out.stdout).ok());

                    let new_mute = mute_output
                        .map(|txt| txt.contains("yes"))
                        .unwrap_or(false);

                    if Some(new_mute) != last_mute {
                        last_mute = Some(new_mute);
                        let _ = tx_mic.send_blocking(OsdEvent::MicMute { muted: new_mute });
                    }
                }
            }
        }
    });

    // to autohide the slider
    let hide_timeout_id: Rc<RefCell<Option<glib::SourceId>>> = Rc::new(RefCell::new(None));

    let hide_timeout_id_clone = hide_timeout_id.clone();
    let osd_box_clone = osd_box.clone();
    let mic_icon = gtk4::Image::from_icon_name("capsule_mic_mute");
    mic_icon.set_pixel_size(24);
    noticapsule.append(&mic_icon);
    mic_icon.set_visible(false);

    let noticapsule_window = ApplicationWindow::new(app);
    noticapsule_window.init_layer_shell();
    noticapsule_window.set_layer(Layer::Top);
    noticapsule_window.set_namespace(Some("capsule"));


    // Main thread: receive and update UI
    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                OsdEvent::Volume { level, muted } => {
                    level_bar.remove_css_class("blight");
                    if muted {
                        level_bar.set_value(0.0);
                        level_bar.remove_css_class("vol");
                        level_bar.add_css_class("muted");
                        osd_box.set_visible(true);
                    } else {
                        level_bar.set_value(level);
                        level_bar.remove_css_class("muted");
                        level_bar.add_css_class("vol");
                        osd_box.set_visible(true);
                    }
                }
                OsdEvent::Brightness { level } => {
                    level_bar.set_value(level);
                    level_bar.remove_css_class("vol");  
                    level_bar.remove_css_class("muted");
                    level_bar.add_css_class("blight");
                    osd_box.set_visible(true);
                }
                OsdEvent::MicMute { muted } => {
                    mic_icon.set_visible(muted);
                }
            }

            // cancel previous hide timeout if any
            if let Some(id) = hide_timeout_id_clone.borrow_mut().take() {
                let _ = id.remove();
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
    
    // No exclusive zone — it's an overlay
    noticapsule_window.set_anchor(Edge::Top, true);
    noticapsule_window.set_anchor(Edge::Right, true);
    noticapsule_window.set_anchor(Edge::Left, true);
    noticapsule_window.set_exclusive_zone(0);
    noticapsule_window.set_decorated(false);
    
    noticapsule_window.set_child(Some(&noticapsule));
    noticapsule_window.show();

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

    let quicky_window = ApplicationWindow::new(app);
    quicky_window.init_layer_shell();
    quicky_window.set_layer(Layer::Top);
    quicky_window.set_namespace(Some("capsule"));

    quicky_window.set_anchor(Edge::Top, true);
    quicky_window.set_anchor(Edge::Bottom, true);
    quicky_window.set_anchor(Edge::Left, true);
    quicky_window.set_exclusive_zone(0);
    quicky_window.set_decorated(false);
    quicky_window.set_child(Some(&boxxy));
    quicky_window.set_width_request(30);

    quicky_window.show();
    
    // desktop window -------------------------------------------------------------------------------------------------------------------------------- //
    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Background);
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

    let desktop_conrtol_box = GtkBox::new(Orientation::Vertical, 0);
    desktop_conrtol_box.set_halign(gtk4::Align::Fill);
    desktop_conrtol_box.set_valign(gtk4::Align::Fill);
    desktop_conrtol_box.set_widget_name("dummy");

    let desktop = GtkBox::new(Orientation::Vertical, 5);
    desktop.set_halign(gtk4::Align::Fill);
    desktop.set_valign(gtk4::Align::Fill);
    desktop.set_vexpand(true);
    desktop.set_hexpand(true);
    desktop.set_margin_top(50);
    desktop.set_margin_bottom(50);
    desktop.set_margin_start(50);
    desktop.set_margin_end(50);
    desktop.set_widget_name("bubble");
    desktop.set_visible(false);
    
    desktop_conrtol_box.append(&desktop);
    desktop_conrtol_box.append(&Label::new(Some("scroll")));

    let desktop_clone = desktop.clone();

    let controllerr = EventControllerScroll::new(EventControllerScrollFlags::VERTICAL);
    controllerr.connect_scroll(move |_, _dx, dy| {
        if dy < 0.0 {
            if desktop_clone.is_visible() {
                desktop_clone.remove_css_class("scale-in");
                desktop_clone.add_css_class("scale-out");

                let widget_clone = desktop_clone.clone();
                glib::timeout_add_local_once(Duration::from_millis(500), move || {
                    widget_clone.set_visible(false);
                    while let Some(child) = widget_clone.first_child() {
                        widget_clone.remove(&child);
                    }
                });
            }
        } else if dy > 0.0 {
            if !desktop_clone.is_visible() {
                desktop_clone.set_visible(true);
                desktoppy::build(desktop_clone.clone());
                desktop_clone.remove_css_class("scale-out");
                desktop_clone.add_css_class("scale-in");
            }
        }

        glib::Propagation::Proceed
    });

    window.add_controller(controllerr);
    window.set_child(Some(&desktop_conrtol_box));
    window.show();
}


fn main() {
    // Start reading from here dumbass

    let app = Application::new(Some("com.ekah.cynideshell"), Default::default());
    app.connect_activate(activate);

    app.run();

}