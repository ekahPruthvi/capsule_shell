use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, Box, Button, CssProvider, Image, Label, Orientation
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use rand::prelude::IndexedRandom;
use glib::timeout_add_local;
use rand::rngs::ThreadRng;
use std::{fs, time::Duration, process};
use std::rc::Rc;
use std::cell::Cell;
use std::fs::File;

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
            background-color:rgba(0, 0, 0, 0);
        }

        #main {
            background-color:rgba(0, 0, 0, 0);

            animation: pulse 1.5s infinite ease-in-out;
        }

        .error_box {
            border-radius: 20px;
            padding: 5px;
            background-color: rgba(34, 34, 34, 0.559);
            border: 2px solid transparent;
            background-image: linear-gradient(rgb(29, 29, 29), rgb(29, 29, 29)),
                                linear-gradient(0deg, rgb(9, 9, 9), rgba(94, 94, 94, 0.686));
            background-origin: border-box;
            background-clip: padding-box, border-box;
            box-shadow: rgba(0, 0, 0, 0.24) 0px 3px 8px;
        }

        #icon_circle {
            background-color: rgba(40, 40, 40, 0);
            border-radius: 50%;
            padding: 5px;
            transition: transform 0.2s;

            animation: shake 0.5s infinite ease-in-out;
        }

        @keyframes pulse {
            0% {
                box-shadow: inset 0 0 0px rgba(255, 0, 102, 0.6);
            }
            50% {
                box-shadow: inset 0 -30px 30px -20px rgba(255, 25, 25, 0.8);
            }
            100% {
                box-shadow: inset 0 0 0px rgba(255, 0, 102, 0.6);
            }
        }

        @keyframes shake {
            0% {
                transform: rotate(-5deg);
            }
            100% {
                transform: rotate(5deg);
            }    
        }


        #title_label {
            font-weight:400;
            font-size: 12px;
            color: rgba(255, 255, 255, 0.71);
        }

        #subtitle_label {
            color: #cccccc72;
            font-size: 12px;
        }

        .ok_button {
            all: unset;
            min-height: 20px;
            min-width: 20px;
            background-color: rgba(251, 251, 251, 0.08);
            color: rgba(198, 198, 198, 0);
            font-size: 1px;
            border-radius: 50px;
            padding: 0px;
            transition: all 300ms ease;
        }
        

        .ok_button:hover {
            background-color:rgba(255, 230, 0, 0.92);
            color: rgb(198, 198, 198);
            font-style: italic;
            font-size: 12px;
            font-weight: 300;
        }

        .ok_button_bye {
            all: unset;
            min-height: 20px;
            min-width: 20px;
            background-color: rgba(251, 251, 251, 0.08);
            color: rgba(198, 198, 198, 0);
            font-size: 1px;
            border-radius: 50px;
            padding: 0px;
            transition: all 300ms ease;
        }
        
        .ok_button_bye:hover {
            background-color:rgba(255, 85, 116, 0.55);
            color: rgb(198, 198, 198);
            font-style: italic;
            font-size: 12px;
            font-weight: 300;
        }

        #shadow {
            color: rgba(255, 0, 0, 0);
            box-shadow: rgba(0, 0, 0, 0.25) 0px 54px 55px, rgba(0, 0, 0, 0.12) 0px -12px 30px, rgba(0, 0, 0, 0.12) 0px 4px 6px, rgba(0, 0, 0, 0.17) 0px 12px 13px, rgba(0, 0, 0, 0.09) 0px -3px 5px;
            border-radius: 25px;
        }

        .osd-hide {
            animation: osd-disappear 0.3s ease-in forwards;
        }

        @keyframes osd-disappear {
            from {
                opacity: 1;
                transform: translateY(0) scale(1);
            }
            to {
                opacity: 0;
                transform: translateY(10px) scale(0.95);
            }
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
    window.fullscreen();
    window.set_decorated(false);
    window.set_namespace(Some("cynidePrsotocols"));
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Right, true);
    window.set_anchor(Edge::Left, true);
    // window.set_margin(Edge::Bottom, 100);
    
    let batt = GtkBox::new(Orientation::Horizontal, 10);
    batt.set_css_classes(&["error_box"]);
    batt.set_hexpand(false);
    batt.set_halign(gtk4::Align::Center);
    batt.set_size_request(100, 20);
    batt.set_margin_bottom(100);

    let current_width_anim = Rc::new(Cell::new(100.0));
    let batt_anim = Rc::new(batt.clone()); 

    let target_width = 500.0;
    let increment_per_frame = 5.0; 

    let current_width_clone = current_width_anim.clone();
    let batt_clone = batt_anim.clone();

    batt_clone.set_width_request(100);

    let exit_button = Button::builder().child(&Label::new(Some(""))).build();
    exit_button.set_tooltip_text(Some("Close"));
    exit_button.set_css_classes(&["ok_button"]);
    exit_button.set_hexpand(true);
    exit_button.set_vexpand(false);
    exit_button.set_halign(gtk4::Align::End);
    exit_button.set_valign(gtk4::Align::Center);
    exit_button.set_margin_end(0);

    let bye_foreva = Button::builder().child(&Label::new(Some(""))).build();
    bye_foreva.set_css_classes(&["ok_button_bye"]);
    bye_foreva.set_tooltip_text(Some("Hide Completely"));
    bye_foreva.set_hexpand(false);
    bye_foreva.set_vexpand(false);
    bye_foreva.set_halign(gtk4::Align::End);
    bye_foreva.set_valign(gtk4::Align::Center);
    bye_foreva.set_margin_end(10);

    let batt_c = batt.clone();
    exit_button.connect_clicked(move |_| {
        batt_c.add_css_class("osd-hide");
        gtk4::glib::timeout_add_local( std::time::Duration::from_millis(300), move || {
            process::exit(0);
            glib::ControlFlow::Break
        });
    }); 

    let batt_b = batt.clone();
    bye_foreva.connect_clicked(move |_| {
        if let Err(e) = File::create("/tmp/batt_no_ask.var") {
            eprintln!("Failed to create file: {}", e);
        }
        batt_b.add_css_class("osd-hide");
        gtk4::glib::timeout_add_local( std::time::Duration::from_millis(300), move || {
            process::exit(0);
            glib::ControlFlow::Break
        });
    });

    let image = Image::from_file("/var/lib/cynager/icons/!con.svg");
    image.set_pixel_size(25);
    let icon_box = Box::new(gtk4::Orientation::Vertical, 0);
    // icon_box.set_hexpand(true);
    icon_box.set_vexpand(true);
    icon_box.set_halign(gtk4::Align::Start);
    icon_box.set_valign(gtk4::Align::Center);
    icon_box.set_widget_name("icon_circle");
    icon_box.append(&image);

    let title = Label::new(Some("Low Battery!"));
    title.set_widget_name("title_label");
    title.set_halign(gtk4::Align::Start);

    let dont_do_this = [
        "get a nuclear reactor!",
        "build a solar farm.",
        "install a hamster-powered turbine!",
        "fly her to the moon.",
        "get static from cat fur.",
        "harness lightning!!",
        "plug into the Matrix.",
        "find pikachu.",
    ];


    let mut rng = ThreadRng::default();
    let random_phrase = dont_do_this
        .choose(&mut rng)
        .unwrap_or(&"do nothing");

    let subtitle_text = format!("Connect charger or {}", random_phrase);
    let subtitle = Label::new(Some(&subtitle_text));
    subtitle.set_justify(gtk4::Justification::Left);
    subtitle.set_widget_name("subtitle_label");

    batt.append(&icon_box);

    let text = GtkBox::new(Orientation::Vertical, 3);
    text.set_vexpand(true);
    text.set_valign(gtk4::Align::Center);
    text.set_margin_end(20);
    text.append(&title);
    text.append(&subtitle);


    gtk4::glib::timeout_add_local(
        std::time::Duration::from_millis(6),
        move || {
            let next_w = current_width_clone.get() + increment_per_frame;
            
            if next_w >= target_width {
                current_width_clone.set(target_width);
                batt_clone.set_width_request(target_width as i32);
                batt_clone.add_css_class("blip");
                batt_clone.append(&text);
                batt_clone.append(&exit_button);
                batt_clone.append(&bye_foreva);
                
                return gtk4::glib::ControlFlow::Break;
            }
            
            current_width_clone.set(next_w);
            batt_clone.set_width_request(next_w as i32);
            
            gtk4::glib::ControlFlow::Continue
        },
    );

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
