use gtk4::{gdk, prelude::*, ApplicationWindow, Box as GtkBox, Button, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use niri_ipc::{socket::Socket, Action, Request, Response};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
struct FocusedGeo {
    x: i32,
    y: i32,
    xsize: i32,
    ysize: i32,
    output: String,
}

#[derive(Debug)]
enum SsdEvent {
    Focused(FocusedGeo),
    NoFocus,
}

pub fn spawn_shelly_side_decorations(app: &gtk4::Application) {
    let (tx, rx) = mpsc::channel::<SsdEvent>();
    let rx = Rc::new(RefCell::new(rx));

    let win = ApplicationWindow::builder()
        .application(app)
        .title("capsuleSSD")
        .build();

    win.init_layer_shell();
    win.set_namespace(Some("shell-side-decorations"));
    win.set_layer(Layer::Overlay);
    win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    win.remove_css_class("background");

    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);

    win.set_margin(Edge::Top, 0);
    win.set_margin(Edge::Left, 0);

    let bar = GtkBox::new(Orientation::Horizontal, 0);
    bar.set_css_classes(&["ssdBar"]);

    // let hover_btn = make_btn("", &["ssdBtn"]);

    // i am lazy to chnage the button names according to wat tey do 

    let btn_close = make_btn(" ", &["ssdBtn", "ssdClose"]);
    let btn_min   = make_btn(" ", &["ssdBtn", "ssdMin"]);
    let btn_float = make_btn(" ", &["ssdBtn", "ssdFloat"]);

    // bar.append(&hover_btn);
    bar.append(&btn_close);
    bar.append(&btn_min);
    bar.append(&btn_float);

    win.set_child(Some(&bar));

    btn_close.connect_clicked(|_| {
        niri_action(Action::CloseWindow { id: None });
    });

    btn_min.connect_clicked(|_| {
        niri_action(Action::ToggleWindowFloating { id: None });
    });

    btn_float.connect_clicked(|_| {
        niri_action(Action::FullscreenWindow { id: None });
    });

    win.set_visible(false);
    win.present();

    std::thread::spawn(move || niri_event_loop(tx));

    let win_weak = win.downgrade();
    let mut current_output: Option<String> = None;

    gtk4::glib::timeout_add_local(Duration::from_millis(16), move || {
        let Some(win) = win_weak.upgrade() else {
            eprintln!("window upgrade broken");
            return gtk4::glib::ControlFlow::Break;
        };

        let rx = rx.borrow();
        let mut last: Option<SsdEvent> = None;
        loop {
            match rx.try_recv() {
                Ok(ev) => last = Some(ev),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return gtk4::glib::ControlFlow::Break;
                }
            }
        }

        if let Some(ev) = last {
            match ev {
                SsdEvent::Focused(geo) => {
                    if current_output.as_deref() != Some(&geo.output) {
                        match find_monitor_by_connector(&geo.output) {
                            Some(monitor) => {
                                win.set_monitor(Some(&monitor));
                                current_output = Some(geo.output.clone());
                            }
                            None => {
                                eprintln!("[ssd] unknown output: {}", geo.output);
                            }
                        }
                    }
                    win.set_margin(Edge::Top,  geo.y + 7);
                    win.set_margin(Edge::Left, geo.x + 7);
                    win.set_visible(true);
                }
                SsdEvent::NoFocus => {
                    win.set_visible(false);
                }
            }
        }

        gtk4::glib::ControlFlow::Continue
    });
}

fn find_monitor_by_connector(connector: &str) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let monitors = display.monitors();
    for i in 0..monitors.n_items() {
        let monitor = monitors
            .item(i)?
            .downcast::<gdk::Monitor>()
            .ok()?;
        if monitor.connector().as_deref() == Some(connector) {
            return Some(monitor);
        }
    }
    None
}

fn make_btn(label: &str, classes: &[&str]) -> Button {
    let b = Button::with_label(label);
    b.set_css_classes(classes);
    b
}

fn niri_action(action: Action) {
    if let Ok(mut sock) = Socket::connect() {
        let _ = sock.send(Request::Action(action));
    }
}

fn niri_event_loop(tx: mpsc::Sender<SsdEvent>) {
    let _ = tx.send(query_focused_geo().map_or(SsdEvent::NoFocus, SsdEvent::Focused));

    let Ok(mut sock) = Socket::connect() else {
        eprintln!("[ssd] failed to connect to niri socket");
        return;
    };

    let Ok(Ok(Response::Handled)) = sock.send(Request::EventStream) else {
        eprintln!("[ssd] niri rejected EventStream");
        return;
    };

    let mut read_event = sock.read_events();
    loop {
        match read_event() {
            Ok(_) => {
                let msg = query_focused_geo()
                    .map_or(SsdEvent::NoFocus, SsdEvent::Focused);
                if tx.send(msg).is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("[ssd] event-stream error: {e}");
                break;
            }
        }
    }
}

fn query_focused_geo() -> Option<FocusedGeo> {
    let workspaces = {
        let mut sock = Socket::connect().ok()?;
        match sock.send(Request::Workspaces) {
            Ok(Ok(Response::Workspaces(ws))) => ws,
            _ => {
                eprintln!("[ssd] failed to query workspaces");
                return None;
            }
        }
    };

    let mut sock = Socket::connect().ok()?;
    match sock.send(Request::FocusedWindow) {
        Ok(Ok(Response::FocusedWindow(Some(w)))) => {
            let (x, y)         = w.layout.tile_pos_in_workspace_view?;
            let (xsize, ysize) = w.layout.window_size;

            let output = w
                .workspace_id
                .and_then(|wid| workspaces.iter().find(|ws| ws.id == wid))
                .and_then(|ws| ws.output.clone())?;

            Some(FocusedGeo {
                x: x as i32,
                y: y as i32,
                xsize: xsize as i32,
                ysize: ysize as i32,
                output,
            })
        }
        _ => None,
    }
}