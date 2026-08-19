use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, Image, Orientation, prelude::*,
};
use gtk4::glib;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{RefCell,Cell};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use serde_json::Value;

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
    let Ok(out) = out else { return map; };
    if !out.status.success() {
        return map;
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return map; };
    let Some(arr) = parsed.as_array() else { return map; };

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

fn make_dock_button(win: &NiriWindow) -> Button {
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
    btn.connect_clicked(move |_| {
        focus_window(id);
    });

    btn
}

fn rebuild_dockbox(dockbox: &GtkBox, windows: &[NiriWindow]) {
    while let Some(child) = dockbox.first_child() {
        dockbox.remove(&child);
    }

    if windows.is_empty() {
        let empty = gtk4::Label::new(Some("No open apps"));
        empty.add_css_class("dockEmpty");
        dockbox.append(&empty);
        return;
    }

    for win in windows {
        let btn = make_dock_button(win);
        dockbox.append(&btn);
    }
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

    let initial = windows_sorted(&get_niri_windows_map());
    rebuild_dockbox(&dockbox_rc, &initial);

    win.set_child(Some(&*dockbox_rc));

    let niri_rx = Rc::new(RefCell::new(spawn_niri_watcher()));

    {
        let dockbox_rc = dockbox_rc.clone();
        let niri_rx = niri_rx.clone();

        glib::timeout_add_local(Duration::from_millis(150), move || {
            let rx = niri_rx.borrow();
            let mut latest: Option<Vec<NiriWindow>> = None;
            while let Ok(windows) = rx.try_recv() {
                latest = Some(windows);
            }
            drop(rx);

            if let Some(windows) = latest {
                rebuild_dockbox(&dockbox_rc, &windows);
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
    hover_ctrl.connect_enter(move |_,_,_| {
        pendingg.set(false);
        pop_enter.remove_css_class("dockleave");
        pop_enter.add_css_class("dockcum");
    });

    dockbox_ctrl.add_controller(hover_ctrl);

    win.present();
    win.set_visible(false);
    win
}