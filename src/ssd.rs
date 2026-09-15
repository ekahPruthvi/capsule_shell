use gtk4::{gdk, prelude::*, ApplicationWindow, Box as GtkBox, Button, GestureDrag, Label, glib, Orientation};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use niri_ipc::{socket::Socket, Action, PositionChange, Request, Response};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;
use chrono::Local;

#[derive(Debug, Clone, PartialEq)]
struct FocusedGeo {
    x: i32,
    y: i32,
    xsize: i32,
    ysize: i32,
    output: String,
    is_floating: bool,
}

#[derive(Debug)]
enum SsdEvent {
    Focused(FocusedGeo),
    NoFocus,
}

const DRAG_THRESHOLD: f64 = 4.0;

pub fn spawn_shelly_side_decorations(app: &gtk4::Application) {
    let (tx, rx) = mpsc::channel::<SsdEvent>();
    let rx = Rc::new(RefCell::new(rx));
    // let alt_held = Rc::new(RefCell::new(false));

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

    let ghost_win = ApplicationWindow::builder()
        .application(app)
        .title("capsuleSSDGhost")
        .build();

    ghost_win.init_layer_shell();
    ghost_win.set_namespace(Some("shell-side-decorations-ghost"));
    ghost_win.set_layer(Layer::Overlay);
    ghost_win.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);
    ghost_win.remove_css_class("background");
    ghost_win.set_anchor(Edge::Top, true);
    ghost_win.set_anchor(Edge::Left, true);
    ghost_win.set_exclusive_zone(-1);

    let ghost_box = GtkBox::new(Orientation::Horizontal, 0);
    ghost_box.set_css_classes(&["ssdDragGhost"]);
    ghost_win.set_child(Some(&ghost_box));
    ghost_win.set_visible(false);

    let bar = GtkBox::new(Orientation::Horizontal, 0);
    bar.set_css_classes(&["ssdBar"]);

    // let hover_btn = make_btn("", &["ssdBtn"]);

    // i am lazy to chnage the button names according to wat tey do 

    let btn_close = make_btn(" ", &["ssdBtn", "ssdClose"]);

    let btn_min   = make_btn(" ", &["ssdBtn", "ssdMin"]);
    btn_min.set_cursor_from_name(Some("grab"));

    let btn_float = make_btn(" ", &["ssdBtn", "ssdFloat"]);

    bar.append(&btn_close);
    bar.append(&btn_min);
    bar.append(&btn_float);

    win.set_child(Some(&bar));

    btn_close.connect_clicked(|_| {
        niri_action(Action::CloseWindow { id: None });
    });

    // {
    //     let alt_held = alt_held.clone();
    //     btn_min.connect_clicked(move |btn| {
    //         let mut held = alt_held.borrow_mut();
    //         if *held {
    //             let _ = std::process::Command::new("ydotool")
    //                 .args(["key", "--key-delay=0", "125:0"])
    //                 .spawn();
    //             *held = false;
    //             btn.remove_css_class("ssdMinActive");
    //         } else {
    //             let _ = std::process::Command::new("ydotool")
    //                 .args(["key", "--key-delay=0", "125:1"])
    //                 .spawn();
    //             *held = true;
    //             btn.add_css_class("ssdMinActive");
    //         }
    //     });
    // }

    // let btn_min_for_sig = btn_min.clone();
    // gtk4::glib::unix_signal_add_local(libc::SIGUSR2, move || {
    //     let mut held = alt_held.borrow_mut();
    //     let _ = std::process::Command::new("ydotool")
    //         .args(["key", "--key-delay=0", "125:0"])
    //         .spawn();
    //     *held = false;
    //     btn_min_for_sig.remove_css_class("ssdMinActive");
    //     gtk4::glib::ControlFlow::Continue
    // });

    btn_float.connect_clicked(|_| {
        niri_action(Action::FullscreenWindow { id: None });
    });

    let timendate = GtkBox::new(Orientation::Horizontal, 5);
    timendate.set_hexpand(true);
    timendate.set_halign(gtk4::Align::Center);
    let time      = Label::new(Some(""));
    time.set_justify(gtk4::Justification::Center);
    time.set_css_classes(&["ampm"]);
    let ampm = Label::new(Some("cynageOS"));
    ampm.set_css_classes(&["ampm"]);

    timendate.append(&time);
    timendate.append(&ampm);

    glib::timeout_add_local(Duration::from_millis(1200), move || {
        let now = Local::now();
        time.set_text(&now.format("%I:%M").to_string());
        ampm.set_text(&now.format(" %p \t %a, %b %e").to_string());
        glib::ControlFlow::Continue
    });

    win.set_visible(false);

 
    let latest_geo: Rc<RefCell<Option<FocusedGeo>>> = Rc::new(RefCell::new(None));
    let current_monitor_rc: Rc<RefCell<Option<gdk::Monitor>>> = Rc::new(RefCell::new(None));

    let win_margin_rc: Rc<RefCell<(i32, i32)>> = Rc::new(RefCell::new((0, 0)));

    std::thread::spawn(move || niri_event_loop(tx));

    {
        let win_weak = win.downgrade();
        let ghost_win_weak = ghost_win.downgrade();
        let latest_geo = latest_geo.clone();
        let win_margin_rc = win_margin_rc.clone();


        let drag_origin: Rc<RefCell<Option<(i32, i32)>>> = Rc::new(RefCell::new(None));
        let dragging: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

        let drag = GestureDrag::new();
        
        drag.connect_drag_begin({
            let latest_geo = latest_geo.clone();
            let win_margin_rc = win_margin_rc.clone();
            let drag_origin = drag_origin.clone();
            let dragging = dragging.clone();
            let btn_min_inner = btn_min.clone();
            move |_gesture, _x, _y| {
                let is_floating = latest_geo
                    .borrow()
                    .as_ref()
                    .map(|g| g.is_floating)
                    .unwrap_or(false);
                if !is_floating {
                    *drag_origin.borrow_mut() = None;
                    return;
                }
                *drag_origin.borrow_mut() = Some(*win_margin_rc.borrow());
                *dragging.borrow_mut() = false;

                
                if let Some(native) = btn_min_inner.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(
                            gdk::Cursor::from_name("none", None).as_ref()
                        );
                    }
                }
            }
        });

        drag.connect_drag_update({
            let win_weak = win_weak.clone();
            let ghost_win_weak = ghost_win_weak.clone();
            let latest_geo = latest_geo.clone();
            let current_monitor_rc = current_monitor_rc.clone();
            let drag_origin = drag_origin.clone();
            let dragging = dragging.clone();
            move |_gesture, dx, dy| {
                let Some((top0, left0)) = *drag_origin.borrow() else {
                    return;
                };
                let Some(win) = win_weak.upgrade() else { return; };

                let dx = dx.round() as i32;
                let dy = dy.round() as i32;

                if !*dragging.borrow() {
                    if dx.abs() < DRAG_THRESHOLD as i32 && dy.abs() < DRAG_THRESHOLD as i32 {
                        return;
                    }
                    *dragging.borrow_mut() = true;

                    if let (Some(ghost_win), Some(geo)) =
                        (ghost_win_weak.upgrade(), latest_geo.borrow().clone())
                    {
                        if let Some(monitor) = current_monitor_rc.borrow().as_ref() {
                            ghost_win.set_monitor(Some(monitor));
                        }
                        ghost_box_set_size(&ghost_win, geo.xsize.max(1), geo.ysize.max(1));
                        ghost_win.set_visible(true);
                    }
                }

                win.set_margin(Edge::Top, top0 + dy);
                win.set_margin(Edge::Left, left0 + dx);

                if let (Some(ghost_win), Some(geo)) =
                    (ghost_win_weak.upgrade(), latest_geo.borrow().clone())
                {
                    ghost_win.set_margin(Edge::Top, geo.y + dy - 4);
                    ghost_win.set_margin(Edge::Left, geo.x + dx - 4);
                }
            }
        });

        drag.connect_drag_end({
            let ghost_win_weak = ghost_win_weak.clone();
            let drag_origin = drag_origin.clone();
            let dragging = dragging.clone();
            let btn_min_done_inner = btn_min.clone();
            move |_gesture, dx, dy| {
                let was_dragging = *dragging.borrow();
                *dragging.borrow_mut() = false;
                *drag_origin.borrow_mut() = None;

                if let Some(ghost_win) = ghost_win_weak.upgrade() {
                    ghost_win.set_visible(false);
                }
                if let Some(native) = btn_min_done_inner.native() {
                    if let Some(surface) = native.surface() {
                        surface.set_cursor(
                            gdk::Cursor::from_name("default", None).as_ref()
                        );
                    }
                }
                if let Some(win) = win_weak.upgrade() {
                    win.set_cursor_from_name(None);
                }

                if !was_dragging {
                    return;
                }

                let dx = dx.round() as i32;
                let dy = dy.round() as i32;
                if dx != 0 || dy != 0 {
                    niri_action(Action::MoveFloatingWindow {
                        id: None,
                        x: PositionChange::AdjustFixed(dx as f64),
                        y: PositionChange::AdjustFixed(dy as f64),
                    });
                }
            }
        });

        btn_min.add_controller(drag);
    }

    let win_weak = win.downgrade();
    let ghost_win_weak = ghost_win.downgrade();
    let bar_for_poll = bar.clone();
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
                    *latest_geo.borrow_mut() = Some(geo.clone());

                    if current_output.as_deref() != Some(&geo.output) {
                        match find_monitor_by_connector(&geo.output) {
                            Some(monitor) => {
                                win.set_monitor(Some(&monitor));
                                if let Some(ghost_win) = ghost_win_weak.upgrade() {
                                    ghost_win.set_monitor(Some(&monitor));
                                }
                                *current_monitor_rc.borrow_mut() = Some(monitor);
                                current_output = Some(geo.output.clone());
                            }
                            None => {
                                eprintln!("[ssd] unknown output: {}", geo.output);
                            }
                        }
                    }
                    if geo.is_floating {
                        win.set_anchor(Edge::Right, false);
                        bar_for_poll.set_size_request(-1, -1);
                        win.set_margin(Edge::Top,  geo.y + 4);
                        win.set_margin(Edge::Left, geo.x + 4);
                        *win_margin_rc.borrow_mut() = (geo.y, geo.x);
                        if timendate.parent().as_ref() == Some(bar.upcast_ref()) {
                            bar.remove(&timendate);
                        }
                        win.set_visible(true);
                    } else {
                        win.set_visible(false);
                        if timendate.parent().as_ref() != Some(bar.upcast_ref()) {
                            bar.append(&timendate);
                        }
                        win.set_anchor(Edge::Right, true);
                        bar_for_poll.set_size_request(-1, -1);
                        win.set_margin(Edge::Top,  geo.y);
                        win.set_margin(Edge::Left, geo.x);
                        *win_margin_rc.borrow_mut() = (geo.y, geo.x);

                        match current_monitor_rc.borrow().as_ref() {
                            Some(_) => {
                                win.set_margin(Edge::Right, 0);
                            }
                            None => {
                                win.set_anchor(Edge::Right, false);
                                bar_for_poll.set_size_request(geo.xsize, -1);
                            }
                        }
                    }
                }
                SsdEvent::NoFocus => {
                    *latest_geo.borrow_mut() = None;
                    win.set_visible(false);
                    if let Some(ghost_win) = ghost_win_weak.upgrade() {
                        ghost_win.set_visible(false);
                    }
                }
            }
        }

        gtk4::glib::ControlFlow::Continue
    });
}

fn ghost_box_set_size(ghost_win: &ApplicationWindow, width: i32, height: i32) {
    if let Some(child) = ghost_win.child() {
        child.set_size_request(width, height);
    }
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
    let w = match sock.send(Request::FocusedWindow) {
        Ok(Ok(Response::FocusedWindow(Some(w)))) => w,
        Ok(Ok(Response::FocusedWindow(None))) => return None, 
        other => {
            eprintln!("[ssd] unexpected FocusedWindow response: {:?}", other);
            return None;
        }
    };

    let is_floating = w.is_floating;

    let (x, y) = w.layout.tile_pos_in_workspace_view.unwrap_or_else(|| {
        (0.0, 0.0)
    });

    let (xsize, ysize) = w.layout.tile_size;

    let Some(output) = w
        .workspace_id
        .and_then(|wid| workspaces.iter().find(|ws| ws.id == wid))
        .and_then(|ws| ws.output.clone())
    else {
        return None;
    };

    Some(FocusedGeo {
        x: x.round() as i32,
        y: y.round() as i32,
        xsize: xsize.round() as i32,
        ysize: ysize.round() as i32,
        output,
        is_floating,
    })
}