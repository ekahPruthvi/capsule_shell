use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, Box, Button, CssProvider, Image, Label, Orientation
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use rand::prelude::IndexedRandom;
use glib::timeout_add_local;
use rand::rngs::ThreadRng;
use std::{fs, time::Duration, process};

fn is_charging() -> bool {
    for i in 0..5 {
        let path = format!("/sys/class/power_supply/BAT{}/status", i);
        if let Ok(status) = fs::read_to_string(&path) {
            if status.trim() == "Charging" {
                return true;
            }
        }
    }
    false
}

fn activate(app: &Application) {
    
    // css auto reload 

    timeout_add_local(Duration::from_millis(100), || {
        if is_charging() {
            println!("Charging detected. Exiting...");
            process::exit(0);
        }
        glib::ControlFlow::Continue
    });

    let css = CssProvider::new();
    css.load_from_data(
        "
        #mainshadow {
            background-color:rgba(0, 0, 0, 0.6);
            background: linear-gradient(
                135deg,
                rgba(18, 18, 18, 0.9) 25%,
                rgba(26, 26, 26, 0.9) 25%,
                rgba(26, 26, 26, 0.9) 50%,
                rgba(18, 18, 18, 0.9) 50%,
                rgba(18, 18, 18, 0.9) 75%,
                rgba(26, 26, 26, 0.9) 75%,
                rgba(26, 26, 26, 0.9)
            );
            background-size: 40px 40px;

            /* Animation */
            animation: move 4s linear infinite;

        }

        @keyframes move {
            0% {
                background-position: 0 0;
            }
            100% {
                background-position: 40px 40px;
            }
        }


        #main {
            background-color:rgba(0, 0, 0, 0);
        }

        #error_box {
            background: rgba(48, 48, 48, 0.17);
            border: 10px solid rgba(0, 0, 0, 0.22);
            background-clip: padding-box;
            border-radius: 25px;
            padding-top: 10px;
            padding-bottom: 20px;
            padding-right: 20px;
            padding-left: 20px;
        }

        #icon_circle {
            background-color: #ff0066;
            border-radius: 48%;
            padding: 20px;
            margin-bottom: 10px;
            animation: pulse 1.5s infinite ease-in-out;
            transition: transform 0.2s;
        }

        @keyframes pulse {
            0% {
                transform: scale(1.0);
                box-shadow: 0 0 0px rgba(255, 0, 102, 0.6);
            }
            50% {
                transform: scale(1.08);
                box-shadow: 0 0 20px rgba(255, 0, 102, 0.8);
            }
            100% {
                transform: scale(1.0);
                box-shadow: 0 0 0px rgba(255, 0, 102, 0.6);
            }
        }


        #title_label {
            font-weight: 900;
            font-size: 20px;
            color: rgb(255, 109, 109);
        }

        #subtitle_label {
            color: #bbbbbb;
            font-size: 14px;
        }

        #ok_button {
            all: unset;
            min-height: 10px;
            min-width: 100px;
            background-color: rgba(255, 255, 255, 0.05);
            color: rgba(0, 0, 0, 0);
            font-size: 2px;
            border-radius: 50px;
            padding: 0px;
            transition: background-color 300ms ease;
        }

        #ok_button:hover {
            background-color:rgba(255, 0, 102, 0.55);
        }

        #shadow {
            color: rgba(255, 0, 0, 0);
            box-shadow: rgba(0, 0, 0, 0.25) 0px 54px 55px, rgba(0, 0, 0, 0.12) 0px -12px 30px, rgba(0, 0, 0, 0.12) 0px 4px 6px, rgba(0, 0, 0, 0.17) 0px 12px 13px, rgba(0, 0, 0, 0.09) 0px -3px 5px;
            border-radius: 25px;
        }
    ",
    );

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let window = ApplicationWindow::new(app);
    window.init_layer_shell();
    window.set_layer(Layer::Top);
    window.auto_exclusive_zone_enable();
    window.fullscreen();
    window.set_decorated(false);
    window.set_namespace(Some("batlow"));

    for (edge, anchor) in [
        (Edge::Left, true),
        (Edge::Right, true),
        (Edge::Top, true),
        (Edge::Bottom, true),
    ] {
        window.set_anchor(edge, anchor);
    }

    let batt = GtkBox::new(Orientation::Vertical, 10);
    batt.set_halign(gtk4::Align::Center);
    batt.set_valign(gtk4::Align::Center);
    batt.set_widget_name("error_box");
    batt.set_size_request(400, 500);

    let exit_button = Button::builder().child(&Label::new(Some("exit"))).build();
    exit_button.set_widget_name("ok_button");
    exit_button.set_hexpand(true);
    exit_button.set_margin_bottom(20);
    exit_button.set_halign(gtk4::Align::Center);
    exit_button.set_valign(gtk4::Align::Start);

    exit_button.connect_clicked(move |_| {
        process::exit(0);
    }); 

    let image = Image::from_icon_name("gnome-power-manager");
    image.set_pixel_size(86);
    let icon_box = Box::new(gtk4::Orientation::Vertical, 0);
    icon_box.set_hexpand(true);
    icon_box.set_vexpand(true);
    icon_box.set_margin_bottom(20);
    icon_box.set_halign(gtk4::Align::Center);
    icon_box.set_valign(gtk4::Align::Center);
    icon_box.set_widget_name("icon_circle");
    icon_box.append(&image);

    let title = Label::new(Some("Low Battery!"));
    title.set_widget_name("title_label");

    let dont_do_this = [
        "get a nuclear reactor",
        "build a solar farm",
        "install a hamster-powered turbine",
        "fly to the moon",
        "get static from cat fur",
        "harness lightning",
        "plug into the Matrix",
    ];


    let mut rng = ThreadRng::default(); // modern thread-local RNG
    let random_phrase = dont_do_this
        .choose(&mut rng)
        .unwrap_or(&"do nothing");

    let subtitle_text = format!("Connect charger or {}", random_phrase);
    let subtitle = Label::new(Some(&subtitle_text));
    subtitle.set_wrap(true);
    subtitle.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    subtitle.set_justify(gtk4::Justification::Center);
    subtitle.set_widget_name("subtitle_label");

    batt.append(&exit_button);
    batt.append(&icon_box);
    batt.append(&title);
    batt.append(&subtitle);

    window.set_child(Some(&batt));
    window.set_widget_name("main");

    let shadow = ApplicationWindow::new(app);
    shadow.init_layer_shell();
    shadow.set_layer(Layer::Top);
    shadow.auto_exclusive_zone_enable();
    shadow.fullscreen();
    shadow.set_decorated(false);
    shadow.set_namespace(Some("batlow_bg"));

    for (edge, anchor) in [
        (Edge::Left, true),
        (Edge::Right, true),
        (Edge::Top, true),
        (Edge::Bottom, true),
    ] {
        shadow.set_anchor(edge, anchor);
    }

    let shadow_box= GtkBox::new(Orientation::Vertical, 0);
    shadow_box.append(&Label::new(Some("this is supposed to be hidden")));
    shadow_box.set_widget_name("shadow");
    shadow_box.set_halign(gtk4::Align::Center);
    shadow_box.set_valign(gtk4::Align::Center);
    shadow_box.set_size_request(400, 500);

    shadow.set_child(Some(&shadow_box));
    shadow.set_widget_name("mainshadow");

    shadow.show();
    window.show();
}

fn main() {
    // Start reading from here dumbass

    let app = Application::new(Some("com.ekah.battlow"), Default::default());
    app.connect_activate(activate);

    app.run();

}