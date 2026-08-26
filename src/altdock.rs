use gtk4::gdk;
use gtk4::glib;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, DragSource, EventControllerMotion,
    Image, prelude::*,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use niri_ipc::{Action, PositionChange, Request, Response, socket::Socket};
use serde_json::Value;

// have to add tray integration 
// rigth click menu, - add to ddesktop, - kill app, - open in settings, - open in files(.desktop) 

#[derive(Debug, Clone, PartialEq)]
pub struct NiriWindow {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
}

fn parse_window(v: &Value) -> Option<NiriWindow> {
    let id = v.get("id")?.as_u64()?;
    let title = v
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    let app_id = v
        .get("app_id")
        .and_then(|a| a.as_str())
        .unwrap_or("")
        .to_string();
    let workspace_id = v.get("workspace_id").and_then(|w| w.as_u64());
    let is_focused = v
        .get("is_focused")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);

    Some(NiriWindow {
        id,
        title,
        app_id,
        workspace_id,
        is_focused,
    })
}

fn windows_sorted(map: &HashMap<u64, NiriWindow>) -> Vec<NiriWindow> {
    let mut list: Vec<NiriWindow> = map.values().cloned().collect();
    list.sort_by(|a, b| {
        a.workspace_id
            .unwrap_or(0)
            .cmp(&b.workspace_id.unwrap_or(0))
            .then(a.id.cmp(&b.id))
    });
    list
}

fn get_niri_windows_map() -> HashMap<u64, NiriWindow> {
    let mut map = HashMap::new();

    let out = Command::new("niri").args(["msg", "-j", "windows"]).output();
    let Ok(out) = out else {
        return map;
    };
    if !out.status.success() {
        return map;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
        return map;
    };
    let Some(arr) = parsed.as_array() else {
        return map;
    };

    for w in arr {
        if let Some(win) = parse_window(w) {
            map.insert(win.id, win);
        }
    }
    map
}

fn focus_window(id: u64) {
    
    let _ = Command::new("niri")
        .args(["msg", "action", "focus-window", "--id", &id.to_string()])
        .spawn();
}

fn get_focused_window_id() -> Option<u64> {
    let out = Command::new("niri")
        .args(["msg", "-j", "focused-window"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let parsed: Value = serde_json::from_str(&text).ok()?;

    if parsed.is_null() {
        return None;
    }

    parsed.get("id")?.as_u64()
}

fn get_window_position(id: u64) -> Option<(f64, f64, bool)> {
    let mut sock = Socket::connect().ok()?;
    match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(windows))) => {
            windows.into_iter().find(|w| w.id == id).and_then(|w| {
                w.layout
                    .tile_pos_in_workspace_view
                    .map(|(x, y)| (x, y, w.is_floating))
            })
        }
        _ => None,
    }
}

fn send_action(action: Action) {
    if let Ok(mut sock) = Socket::connect() {
        let _ = sock.send(Request::Action(action));
    }
}

fn nudge_window(id: u64, offset_y: f64, on_done: impl Fn() + 'static) {
    let Some((x, y, is_floating)) = get_window_position(id) else {
        on_done();
        return;
    };

    if !is_floating {
        on_done();
        return;
    }

    const OUT_STEPS: u32 = 5;
    const BACK_STEPS: u32 = 5;
    const TICK_MS: u64 = 16; // ~60fps
    let target_y = y + offset_y;
    let step = Rc::new(Cell::new(0u32));

    glib::timeout_add_local(Duration::from_millis(TICK_MS), move || {
        let s = step.get();
        let total = OUT_STEPS + BACK_STEPS;

        if s >= total {
            send_action(Action::MoveFloatingWindow {
                id: Some(id),
                x: PositionChange::SetFixed(x),
                y: PositionChange::SetFixed(y),
            });
            on_done();
            return glib::ControlFlow::Break;
        }

        let cur_y = if s < OUT_STEPS {
            let progress = s as f64 / OUT_STEPS as f64;
            let t = 1.0 - (1.0 - progress).powi(3);
            y + (target_y - y) * t
        } else {
            let progress = (s - OUT_STEPS) as f64 / BACK_STEPS as f64;
            let t = 1.0 - (1.0 - progress).powi(3);
            target_y + (y - target_y) * t
        };

        send_action(Action::MoveFloatingWindow {
            id: Some(id),
            x: PositionChange::SetFixed(x),
            y: PositionChange::SetFixed(cur_y),
        });

        step.set(s + 1);
        glib::ControlFlow::Continue
    });
}

fn focus_window_animated(id: u64) {
    let Some(orig_id) = get_focused_window_id() else {
        focus_window(id);
        return;
    };

    if orig_id == id {
        focus_window(id);
        return;
    }

    nudge_window(orig_id, 300.0, || {});
    nudge_window(id, -300.0, move || {
        focus_window(id);
    });
}

fn preview_window(
    id: u64,
    hover_ctrl: EventControllerMotion,
    prev_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
    committed: Rc<Cell<bool>>,
) {
    let Some(orig_id) = get_focused_window_id() else {
        return;
    };

    if orig_id == id {
        return;
    }

    committed.set(false);

    focus_window(id);

    let handler_slot = prev_handler.clone();
    let committed_leave = committed.clone();

    let handler_id = hover_ctrl.connect_leave(move |_| {
        if committed_leave.get() {
            *handler_slot.borrow_mut() = None;
            return;
        }

        focus_window(orig_id);
        *handler_slot.borrow_mut() = None;
    });

    *prev_handler.borrow_mut() = Some(handler_id);
}

pub fn spawn_niri_watcher() -> std::sync::mpsc::Receiver<Vec<NiriWindow>> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<NiriWindow>>();

    std::thread::spawn(move || {
        let mut windows = get_niri_windows_map();
        if tx.send(windows_sorted(&windows)).is_err() {
            return;
        }

        loop {
            let child = Command::new("niri")
                .args(["msg", "-j", "event-stream"])
                .stdout(Stdio::piped())
                .spawn();

            let Ok(mut child) = child else {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            };
            let Some(stdout) = child.stdout.take() else {
                std::thread::sleep(Duration::from_secs(2));
                continue;
            };

            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let Ok(val) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };

                let mut changed = false;

                if let Some(obj) = val.get("WindowsChanged") {
                    if let Some(arr) = obj.get("windows").and_then(|w| w.as_array()) {
                        windows.clear();
                        for w in arr {
                            if let Some(win) = parse_window(w) {
                                windows.insert(win.id, win);
                            }
                        }
                        changed = true;
                    }
                } else if let Some(obj) = val.get("WindowOpenedOrChanged") {
                    if let Some(w) = obj.get("window") {
                        if let Some(win) = parse_window(w) {
                            windows.insert(win.id, win);
                            changed = true;
                        }
                    }
                } else if let Some(obj) = val.get("WindowClosed") {
                    if let Some(id) = obj.get("id").and_then(|v| v.as_u64()) {
                        windows.remove(&id);
                        changed = true;
                    }
                } else if let Some(obj) = val.get("WindowFocusChanged") {
                    let focused_id = obj.get("id").and_then(|v| v.as_u64());
                    for (id, w) in windows.iter_mut() {
                        w.is_focused = Some(*id) == focused_id;
                    }
                    changed = true;
                }

                if changed && tx.send(windows_sorted(&windows)).is_err() {
                    return;
                }
            }

            let _ = child.kill();
            windows = get_niri_windows_map();
            if tx.send(windows_sorted(&windows)).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });

    rx
}

fn desktop_for_app_id(app_id: &str) -> Option<std::path::PathBuf> {
    if app_id.is_empty() {
        return None;
    }

    let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        search_dirs.push(std::path::Path::new(&home).join(".local/share/applications"));
    }
    if let Some(xdg_data_dirs) = std::env::var_os("XDG_DATA_DIRS") {
        for dir in std::env::split_paths(&xdg_data_dirs) {
            search_dirs.push(dir.join("applications"));
        }
    }
    search_dirs.push(std::path::PathBuf::from("/usr/share/applications"));
    search_dirs.push(std::path::PathBuf::from("/usr/local/share/applications"));

    for dir in search_dirs {
        let candidate = dir.join(format!("{app_id}.desktop"));
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    None
}

fn make_dock_btn(win: &NiriWindow) -> Button {
    let icon_name = if win.app_id.is_empty() {
        "application-x-executable".to_string()
    } else {
        win.app_id.clone()
    };

    let icon = Image::from_icon_name(&icon_name);
    icon.set_pixel_size(62);

    let label_text = if win.title.is_empty() {
        win.app_id.clone()
    } else {
        win.title.clone()
    };

    let btn = Button::builder()
        .child(&icon)
        .css_classes(["dockBtn"])
        .tooltip_text(&label_text)
        .build();

    if win.is_focused {
        btn.add_css_class("dockBtnActive");
    }

    let id = win.id;
    let hover_ctrl = gtk4::EventControllerMotion::new();
    let pending: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    let prev_preview_handler: Rc<RefCell<Option<glib::SignalHandlerId>>> =
        Rc::new(RefCell::new(None));
    let committed = Rc::new(Cell::new(false));

    let click_gesture = gtk4::GestureClick::new();
    let click_committed = committed.clone();

    click_gesture.connect_pressed(move |_, _, _, _| {
        click_committed.set(true);
    });

    btn.add_controller(click_gesture);

    let click_prev_handler = prev_preview_handler.clone();
    let click_hover_ctrl = hover_ctrl.clone();
    let click_pending = pending.clone();
    btn.connect_clicked(move |_| {
        click_pending.set(false);
        if let Some(handler_id) = click_prev_handler.borrow_mut().take() {
            click_hover_ctrl.disconnect(handler_id);
        }

        focus_window_animated(id);
    });

    let pending_enter = Rc::clone(&pending);
    let committed_enter = committed.clone();
    hover_ctrl.connect_enter(move |hvr_ctrl, _, _| {
        pending_enter.set(true);
        let pending_timeout = Rc::clone(&pending_enter);
        let hvr = hvr_ctrl.clone();
        let prev_handler = prev_preview_handler.clone();
        let committed = committed_enter.clone();
        glib::timeout_add_local(Duration::from_millis(1000), move || {
            if pending_timeout.get() {
                preview_window(id, hvr.clone(), prev_handler.clone(), committed.clone());
            }
            glib::ControlFlow::Break
        });
    });

    let pending_leave = Rc::clone(&pending);
    hover_ctrl.connect_leave(move |_| {
        pending_leave.set(false);
    });

    btn.add_controller(hover_ctrl);

    if let Some(desktop_path) = desktop_for_app_id(&win.app_id) {
        let drag_source = DragSource::new();
        drag_source.set_actions(gdk::DragAction::COPY);

        let prepare_path = desktop_path.clone();
        drag_source.connect_prepare(move |_src, _x, _y| {
            let uri = glib::filename_to_uri(&prepare_path, None).ok()?;
            let payload = format!("{uri}\r\n");
            let bytes = glib::Bytes::from_owned(payload.into_bytes());
            Some(gdk::ContentProvider::for_bytes("text/uri-list", &bytes))
        });

        
        let drag_icon = icon.clone();

        if let Some(settings) = gtk4::Settings::default() {
            settings.set_gtk_dnd_drag_threshold(24);
        }

        drag_source.connect_drag_begin(move |src, _drag| {
            if let Some(paintable) = drag_icon.paintable() {
                src.set_icon(Some(&paintable), 0, 0);
            }
            if let Err(e) = std::process::Command::new("pkill")
                .args(["-USR1", "capsule"])
                .spawn()
            {
                eprintln!("[appd] Failed to launch show-desktop: {}", e);
            }
        });

        icon.add_controller(drag_source);
    }

    btn
}

#[derive(Default)]
struct DockState {
    buttons: HashMap<u64, Button>,
    order: Vec<u64>,
}

fn update_dockbox(dockbox: &GtkBox, state: &Rc<RefCell<DockState>>, windows: &[NiriWindow]) {
    let mut state = state.borrow_mut();
    let new_order: Vec<u64> = windows.iter().map(|w| w.id).collect();

    if !windows.is_empty() && new_order == state.order {
        for win in windows {
            if let Some(btn) = state.buttons.get(&win.id) {
                if win.is_focused {
                    btn.add_css_class("dockBtnActive");
                } else {
                    btn.remove_css_class("dockBtnActive");
                }
                let label_text = if win.title.is_empty() {
                    win.app_id.clone()
                } else {
                    win.title.clone()
                };
                btn.set_tooltip_text(Some(&label_text));
            }
        }
        return;
    }

    while let Some(child) = dockbox.first_child() {
        dockbox.remove(&child);
    }
    state.buttons.clear();

    if windows.is_empty() {
        let empty = gtk4::Label::new(Some("No open apps"));
        empty.add_css_class("dockEmpty");
        dockbox.append(&empty);
        state.order.clear();
        return;
    }

    for win in windows {
        let btn = make_dock_btn(win);
        dockbox.append(&btn);
        state.buttons.insert(win.id, btn);
    }
    state.order = new_order;
}

pub fn spawn_altdock(app: &Application, dockbox: GtkBox) -> ApplicationWindow {
    let win = ApplicationWindow::builder()
        .application(app)
        .title("AltDock")
        .css_classes(["dockOverlay"])
        .build();

    win.init_layer_shell();
    win.set_namespace(Some("AltDock"));
    win.set_layer(Layer::Top);
    win.remove_css_class("background");
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, false);
    win.set_anchor(Edge::Right, false);
    win.set_anchor(Edge::Bottom, false);

    let dockbox_ctrl = dockbox.clone();
    let dockbox_clone_anim = dockbox.clone();

    let dockbox_rc = Rc::new(dockbox);
    let dock_state: Rc<RefCell<DockState>> = Rc::new(RefCell::new(DockState::default()));

    let initial = windows_sorted(&get_niri_windows_map());
    update_dockbox(&dockbox_rc, &dock_state, &initial);

    win.set_child(Some(&*dockbox_rc));

    let niri_rx = Rc::new(RefCell::new(spawn_niri_watcher()));

    {
        let dockbox_rc = dockbox_rc.clone();
        let niri_rx = niri_rx.clone();
        let dock_state = dock_state.clone();

        glib::timeout_add_local(Duration::from_millis(150), move || {
            let rx = niri_rx.borrow();
            let mut latest: Option<Vec<NiriWindow>> = None;
            while let Ok(windows) = rx.try_recv() {
                latest = Some(windows);
            }
            drop(rx);

            if let Some(windows) = latest {
                update_dockbox(&dockbox_rc, &dock_state, &windows);
            }

            glib::ControlFlow::Continue
        });
    }

    let hover_ctrl = gtk4::EventControllerMotion::new();
    let pending: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let pending_leavein = Rc::clone(&pending);
    let pop_leave = win.clone();
    hover_ctrl.connect_leave(move |_| {
        pending_leavein.set(true);
        let pop = pop_leave.clone();
        let pending_hide = Rc::clone(&pending_leavein);
        let dockbox_clone_anim = dockbox_clone_anim.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if pending_hide.get() {
                dockbox_clone_anim.remove_css_class("dockcum");
                dockbox_clone_anim.add_css_class("dockleave");

                let pop2 = pop.clone();
                let pending_hide2 = Rc::clone(&pending_hide);
                glib::timeout_add_local(Duration::from_millis(400), move || {
                    if pending_hide2.get() {
                        pop2.set_visible(false);
                    }
                    glib::ControlFlow::Break
                });
            }
            glib::ControlFlow::Break
        });
    });

    let pendingg = Rc::clone(&pending);
    let pop_enter = win.clone();
    hover_ctrl.connect_enter(move |_, _, _| {
        pendingg.set(false);
        pop_enter.remove_css_class("dockleave");
        pop_enter.add_css_class("dockcum");
    });

    dockbox_ctrl.add_controller(hover_ctrl);

    win.present();
    win.set_visible(false);
    win
}