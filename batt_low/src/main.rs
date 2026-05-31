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
    
    // timeout_add_local(Duration::from_millis(100), || {
    //     if is_charging() {
    //         println!("Charging detected. Exiting...");
    //         process::exit(0);
    //     }
    //     glib::ControlFlow::Continue
    // });

    let css = CssProvider::new();
    css.load_from_data(
        "
        #mainshadow {
            background-color:rgba(0, 0, 0, 0);
        }

        #main {
            background-color:rgba(0, 0, 0, 0);
        }

        #error_box {
            background: rgba(0, 0, 0, 0.22);
            background-clip: padding-box;
            border-radius: 25px;
            padding-top: 10px;
            padding-bottom: 20px;
            padding-right: 20px;
            padding-left: 20px;
        }

        #icon_circle {
            // background-color: #ff0066;
            background-color: rgb(40, 40, 40);
            border-radius: 50%;
            padding: 20px;
            animation: pulse 1.5s infinite ease-in-out;
            transition: transform 0.2s;
        }

        @keyframes pulse {
            0% {
                box-shadow: 0 0 0px rgba(255, 0, 102, 0.6);
            }
            50% {
                box-shadow: 0 0 50px rgba(255, 0, 102, 0.8);
            }
            100% {
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
    window.set_layer(Layer::Overlay);
    // window.auto_exclusive_zone_enable();
    window.fullscreen();
    window.set_decorated(false);
    window.set_namespace(Some("cynideProtocols"));
    window.set_anchor(Edge::Bottom, true);
    window.set_margin(Edge::Bottom, 100);
    
    let batt = GtkBox::new(Orientation::Horizontal, 10);
    batt.set_halign(gtk4::Align::Center);
    batt.set_valign(gtk4::Align::Center);
    batt.set_widget_name("error_box");
    batt.set_size_request(400, 50);

    let exit_button = Button::builder().child(&Label::new(Some("exit"))).build();
    exit_button.set_widget_name("ok_button");
    exit_button.set_hexpand(true);
    exit_button.set_margin_bottom(20);
    exit_button.set_halign(gtk4::Align::Center);
    exit_button.set_valign(gtk4::Align::Start);

    exit_button.connect_clicked(move |_| {
        process::exit(0);
    }); 

    let image = Image::from_file("/var/lib/cynager/icons/!con.svg");
    image.set_pixel_size(44);
    let icon_box = Box::new(gtk4::Orientation::Vertical, 0);
    icon_box.set_hexpand(true);
    icon_box.set_vexpand(true);
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
        "fly her to the moon",
        "get static from cat fur",
        "harness lightning",
        "plug into the Matrix",
    ];


    let mut rng = ThreadRng::default();
    let random_phrase = dont_do_this
        .choose(&mut rng)
        .unwrap_or(&"do nothing");

    let subtitle_text = format!("Connect charger or {}", random_phrase);
    let subtitle = Label::new(Some(&subtitle_text));
    subtitle.set_wrap(true);
    subtitle.set_wrap_mode(gtk4::pango::WrapMode::WordChar);
    subtitle.set_justify(gtk4::Justification::Center);
    subtitle.set_widget_name("subtitle_label");

    batt.append(&icon_box);
    batt.append(&title);
    batt.append(&subtitle);
    batt.append(&exit_button);

    window.set_child(Some(&batt));
    window.set_widget_name("main");

    window.show();
}

fn main() {
    // Start reading from here dumbass

    let app = Application::new(Some("ekah.scu.battlow"), Default::default());
    app.connect_activate(activate);

    app.run();

}
