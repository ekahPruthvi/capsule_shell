use gtk4::{
    glib, prelude::*, Align, Application, ApplicationWindow, Box as GtkBox, Calendar, CssProvider, Label, Orientation, EventControllerMotion
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use gtk4::gdk::Display;
use chrono::Local;
use gtk4::glib::timeout_add_seconds_local;
use std::cell::RefCell;
use std::rc::Rc;


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
    vbox.set_widget_name("bob");
    window.set_child(Some(&vbox));

    window.show();
}

fn main() {
    let app = Application::new(Some("com.ekah.cynideshell"), Default::default());
    app.connect_activate(activate);
    app.run();
}
