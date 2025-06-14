use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, Button, Image, LevelBar
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use chrono::Local;
use gtk4::glib::timeout_add_seconds_local;
use std::cell::RefCell;
use std::rc::Rc;
use std::fs;
use std::process::Command;
use std::time::UNIX_EPOCH;
use std::path::Path;
use gtk4::glib::ControlFlow::Continue;

pub fn start_status_icon_updater(container: &Rc<GtkBox>) {
    // Initial population
    append_status_icons(container);

    // Set interval to refresh every 10 seconds
    let container_clone = container.clone();
    glib::timeout_add_seconds_local(10, move || {
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


fn activate(app: &Application) {
    
    // Main full-screen dashboard
    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Background);
    window.auto_exclusive_zone_enable();
    window.fullscreen();
    window.set_decorated(false);
    window.set_namespace(Some("cynide"));

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
            font-size: 100px;
            font-weight: 900;
            color: white;
        }

        label.date {
            font-size: 16px;
            color: #cccccc;
        }

        window {
            background-color: rgba(20, 20, 20, 0);
        }

        #bob {
            background-color: rgba(0, 0, 0, 0.2); 
            padding-top: 100px; 
            padding-bottom: 100px;
            padding-right: 20px;
            padding-left: 20px;  
            border-radius: 12px; 
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
            background-color: rgba(255, 255, 255, 0.06);
            transition: background-color 0.2s ease, transform 0.2s ease;
            transform: scale(1.0);
        }

        button.qlicons:hover {
            background-color: rgba(49, 49, 49, 0);
            transform: scale(1.5);
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
    ",
    );

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let timedatebox = GtkBox::new(Orientation::Vertical, 30);
    timedatebox.set_halign(gtk4::Align::Center);
    timedatebox.set_valign(gtk4::Align::Center);

    let qlbox = GtkBox::new(Orientation::Vertical, 0);
    qlbox.set_widget_name("qlbar");

    let commands = Rc::new(RefCell::new(Vec::new()));
    let last_hash = Rc::new(RefCell::new(0u64));

    // Initial population
    ql_creator(&qlbox, commands.clone(), last_hash.clone());

    // Poll every 1s to check for updates
    {
        let boxxy_clone = qlbox.clone();
        timeout_add_seconds_local(1, move || {
            ql_creator(&boxxy_clone, commands.clone(), last_hash.clone());
            glib::ControlFlow::Continue
        });
    }

    let boxxy = GtkBox::new(Orientation::Vertical, 2);
    boxxy.set_valign(gtk4::Align::Center);
    boxxy.set_halign(gtk4::Align::End);
    boxxy.set_margin_start(10);
    boxxy.set_widget_name("cynbar");
    
    let status_box = Rc::new(GtkBox::new(gtk4::Orientation::Vertical, 5));
    start_status_icon_updater(&status_box);
    
    boxxy.append(&qlbox);
    boxxy.append(&*status_box);

    let time_label = Label::new(None);
    time_label.set_css_classes(&["time"]);

    let date_label = Label::new(None);
    date_label.set_css_classes(&["date"]);

    let now = Local::now();
    time_label.set_text(&now.format("%I\n%M").to_string());
    date_label.set_text(&now.format("%p\n%d %B %Y,\n%A").to_string());

    let time_label_ref = Rc::new(RefCell::new(time_label));
    timeout_add_seconds_local(1, {
        let time_label = time_label_ref.clone();
        move || {
            let now = Local::now();
            time_label.borrow().set_text(&now.format("%I\n%M").to_string());
            glib::ControlFlow::Continue
        }
    });

    timedatebox.append(&*time_label_ref.borrow());
    timedatebox.append(&date_label);
    timedatebox.set_widget_name("bob");
    timedatebox.set_halign(gtk4::Align::Center);
    

    let dbox = GtkBox::new(Orientation::Horizontal, 10);

    let hdummy_start = GtkBox::new(Orientation::Horizontal, 0);
    hdummy_start.set_hexpand(true);


    let hdummy_end = GtkBox::new(Orientation::Horizontal, 0);
    hdummy_end.set_hexpand(true);

    // dbox.set_halign(gtk4::Align::Center);
    dbox.set_valign(gtk4::Align::Center);

    dbox.append(&boxxy);
    dbox.append(&hdummy_start);
    dbox.append(&timedatebox);
    dbox.append(&hdummy_end);

    window.set_child(Some(&dbox));

    window.show();
}

fn main() {
    let app = Application::new(Some("com.ekah.cynideshell"), Default::default());
    app.connect_activate(activate);
    app.run();
}