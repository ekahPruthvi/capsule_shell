use gtk4::{
    glib, prelude::*, Application, ApplicationWindow, Box as GtkBox, CssProvider, Label, Orientation, EventControllerMotion, Button, Image
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
                container.append(&button);

                exec = None;
                icon = None;
            }
        }
    }
     // Ensure new widgets are visible
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

        #bob{
            background-color: rgba(0, 0, 0, 0.2); 
            padding: 10px;  
            border-radius: 10px; 
        }

    ",
    );

    gtk4::style_context_add_provider_for_display(
        &Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let vbox = GtkBox::new(Orientation::Vertical, 30);
    vbox.set_vexpand(true);
    vbox.set_margin_bottom(200);
    vbox.set_margin_top(200);
    vbox.set_margin_end(100);
    vbox.set_margin_start(100);

    let boxxy = GtkBox::new(Orientation::Horizontal, 0);
    let commands = Rc::new(RefCell::new(Vec::new()));
    let last_hash = Rc::new(RefCell::new(0u64));

    // Initial population
    ql_creator(&boxxy, commands.clone(), last_hash.clone());

    // Poll every 1s to check for updates
    {
        let boxxy_clone = boxxy.clone();
        timeout_add_seconds_local(1, move || {
            ql_creator(&boxxy_clone, commands.clone(), last_hash.clone());
            glib::ControlFlow::Continue
        });
    }

    let time_label = Label::new(None);
    time_label.set_css_classes(&["time"]);

    let date_label = Label::new(None);
    date_label.set_css_classes(&["date"]);

    let now = Local::now();
    time_label.set_text(&now.format("%I\n%M").to_string());
    date_label.set_text(&now.format("%p\n%A, %d %B %Y").to_string());

    let time_label_ref = Rc::new(RefCell::new(time_label));
    timeout_add_seconds_local(1, {
        let time_label = time_label_ref.clone();
        move || {
            let now = Local::now();
            time_label.borrow().set_text(&now.format("%I\n%M").to_string());
            glib::ControlFlow::Continue
        }
    });

    // Add widgets
    vbox.append(&*time_label_ref.borrow());
    vbox.append(&date_label);
    vbox.append(&boxxy);
    vbox.set_widget_name("bob");
    window.set_child(Some(&vbox));

    window.show();
}

fn main() {
    let app = Application::new(Some("com.ekah.cynideshell"), Default::default());
    app.connect_activate(activate);
    app.run();
}
