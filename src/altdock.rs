use gtk4::gdk;
use gtk4::glib;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, DragSource,
    GestureClick, Image, Popover, Separator, prelude::*,
};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io::BufRead;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::time::Duration;

use niri_ipc::{
    Action, PositionChange, Request, Response, SizeChange, WorkspaceReferenceArg, socket::Socket,
};
use serde_json::Value;

use system_tray::client::{ActivateRequest, Client as TrayClient};
use system_tray::item::StatusNotifierItem;
use system_tray::menu::{
    MenuItem as TrayMenuItem, MenuType as TrayMenuType, ToggleState as TrayToggleState, TrayMenu,
};

use crate::desktopstat::cbq_ifdesktop;

#[derive(Debug, Clone, PartialEq)]
pub struct NiriWindow {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace_id: Option<u64>,
    pub is_focused: bool,
    pub is_floating: bool,
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
    let is_floating = v
        .get("is_floating")
        .and_then(|f| f.as_bool())
        .unwrap_or(false);

    Some(NiriWindow {
        id,
        title,
        app_id,
        workspace_id,
        is_focused,
        is_floating,
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

thread_local! {
    static DRAGGED_WINDOW: Cell<Option<u64>> = const { Cell::new(None) };
    static SPLIT_ORIGINS: RefCell<HashMap<u64, Option<u64>>> = RefCell::new(HashMap::new());
    static DESKTOP_SHOWN_BY_DRAG: Cell<bool> = const { Cell::new(false) };
}

fn toggle_show_desktop() {
    if let Err(e) = std::process::Command::new("pkill")
        .args(["-USR1", "capsule"])
        .spawn()
    {
        eprintln!("[altdock] Failed to toggle show-desktop: {}", e);
    }
}

fn tile_pair_in_new_workspace(target: u64, dragged: u64) {
    if target == dragged {
        return;
    }
    let Ok(mut sock) = Socket::connect() else {
        return;
    };
    let windows = match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(w))) => w,
        _ => return,
    };
    let workspaces = match sock.send(Request::Workspaces) {
        Ok(Ok(Response::Workspaces(w))) => w,
        _ => return,
    };

    let (Some(wt), Some(wd)) = (
        windows.iter().find(|w| w.id == target),
        windows.iter().find(|w| w.id == dragged),
    ) else {
        return;
    };

    let is_tiled = |w: &niri_ipc::Window| {
        !w.is_floating
            && windows
                .iter()
                .any(|o| o.id != w.id && !o.is_floating && o.workspace_id == w.workspace_id)
    };
    let join = if is_tiled(wt) {
        Some((wt, wd))
    } else if is_tiled(wd) {
        Some((wd, wt))
    } else {
        None
    };
    if let Some((anchor, mover)) = join {
        let anchor_ws = anchor
            .workspace_id
            .and_then(|id| workspaces.iter().find(|ws| ws.id == id));
        if let Some(ws) = anchor_ws {
            if mover.workspace_id != anchor.workspace_id {
                SPLIT_ORIGINS.with(|m| {
                    m.borrow_mut()
                        .entry(mover.id)
                        .or_insert(mover.workspace_id);
                });
                send_action_checked(
                    "focus-workspace",
                    Action::FocusWorkspace {
                        reference: WorkspaceReferenceArg::Index(ws.idx),
                    },
                );
                if mover.is_floating {
                    send_action(Action::MoveWindowToTiling { id: Some(mover.id) });
                }
                move_window_to_ws(mover.id, ws.id, ws.idx);
            } else if mover.is_floating {
                send_action(Action::MoveWindowToTiling { id: Some(mover.id) });
            }
            send_action_checked(
                "set-window-width",
                Action::SetWindowWidth {
                    id: Some(mover.id),
                    change: SizeChange::SetProportion(50.0),
                },
            );
            send_action(Action::FocusWindow { id: mover.id });
            return;
        }
    }

    let Some(current) = workspaces.iter().find(|ws| ws.is_focused) else {
        eprintln!("[altdock] no focused workspace found for split");
        return;
    };
    let Some((new_ws, new_ws_idx)) = workspaces
        .iter()
        .find(|ws| ws.output == current.output && ws.idx == current.idx + 1)
        .map(|ws| (ws.id, ws.idx))
    else {
        eprintln!(
            "[altdock] no workspace after idx {} to split into",
            current.idx
        );
        return;
    };
    eprintln!(
        "[altdock] split {} + {} -> workspace id {} (idx {})",
        target, dragged, new_ws, new_ws_idx
    );

    SPLIT_ORIGINS.with(|m| {
        let mut m = m.borrow_mut();
        m.entry(wt.id).or_insert(wt.workspace_id);
        m.entry(wd.id).or_insert(wd.workspace_id);
    });

    send_action_checked(
        "focus-workspace",
        Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Index(new_ws_idx),
        },
    );

    for w in [wt, wd] {
        if w.is_floating {
            send_action(Action::MoveWindowToTiling { id: Some(w.id) });
        }
        move_window_to_ws(w.id, new_ws, new_ws_idx);
        send_action_checked(
            "set-window-width",
            Action::SetWindowWidth {
                id: Some(w.id),
                change: SizeChange::SetProportion(50.0),
            },
        );
    }
    send_action(Action::FocusWindow { id: target });
}

fn restore_split(id: u64) {
    let Ok(mut sock) = Socket::connect() else {
        return;
    };
    let windows = match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(w))) => w,
        _ => return,
    };
    let workspaces = match sock.send(Request::Workspaces) {
        Ok(Ok(Response::Workspaces(w))) => w,
        _ => return,
    };

    let Some(current_ws) = windows.iter().find(|w| w.id == id).and_then(|w| w.workspace_id)
    else {
        return;
    };

    let mut focus_ws: Option<u64> = None;
    for w in windows.iter().filter(|w| w.workspace_id == Some(current_ws)) {
        let origin = SPLIT_ORIGINS.with(|m| m.borrow_mut().remove(&w.id));
        let Some(Some(origin_ws)) = origin else {
            continue;
        };
        if !workspaces.iter().any(|ws| ws.id == origin_ws) {
            continue;
        }
        send_action(Action::MoveWindowToWorkspace {
            window_id: Some(w.id),
            reference: WorkspaceReferenceArg::Id(origin_ws),
            focus: false,
        });
        if w.id == id || focus_ws.is_none() {
            focus_ws = Some(origin_ws);
        }
    }
    if let Some(ws) = focus_ws {
        send_action(Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Id(ws),
        });
    }
}

fn send_action_checked(label: &str, action: Action) -> bool {
    let Ok(mut sock) = Socket::connect() else {
        eprintln!("[altdock] {label}: could not connect to niri socket");
        return false;
    };
    match sock.send(Request::Action(action)) {
        Ok(Ok(_)) => true,
        Ok(Err(e)) => {
            eprintln!("[altdock] {label}: niri rejected action: {e}");
            false
        }
        Err(e) => {
            eprintln!("[altdock] {label}: ipc error: {e}");
            false
        }
    }
}

fn move_window_to_ws(win_id: u64, ws_id: u64, ws_idx: u8) {
    let ok = send_action_checked(
        "move-window-to-workspace (index)",
        Action::MoveWindowToWorkspace {
            window_id: Some(win_id),
            reference: WorkspaceReferenceArg::Index(ws_idx),
            focus: false,
        },
    );
    if !ok {
        send_action_checked(
            "move-window-to-workspace (id)",
            Action::MoveWindowToWorkspace {
                window_id: Some(win_id),
                reference: WorkspaceReferenceArg::Id(ws_id),
                focus: false,
            },
        );
    }

    if let Ok(mut sock) = Socket::connect() {
        if let Ok(Ok(Response::Windows(ws))) = sock.send(Request::Windows) {
            let now = ws.iter().find(|w| w.id == win_id).and_then(|w| w.workspace_id);
            eprintln!(
                "[altdock] window {win_id} now on workspace {:?} (wanted {ws_id})",
                now
            );
        }
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

    const OUT_STEPS: u32 = 12;
    const BACK_STEPS: u32 = 12;
    const TICK_US: u64 = 6_944;
    let target_y = y + offset_y;
    let step = Rc::new(Cell::new(0u32));

    glib::timeout_add_local(Duration::from_micros(TICK_US), move || {
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

    if cbq_ifdesktop() {
        if let Err(e) = std::process::Command::new("pkill")
            .args(["-USR1", "capsule"])
            .spawn()
        {
            eprintln!("[altdock] Failed to launch show-desktop before focusing window: {}", e);
        }
    }
    nudge_window(orig_id, 300.0, || {});
    nudge_window(id, -300.0, move || {
        focus_window(id);
    });
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
                            if win.is_focused {
                                for (other_id, other) in windows.iter_mut() {
                                    if *other_id != win.id {
                                        other.is_focused = false;
                                    }
                                }
                            }
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

#[derive(Clone)]
struct TrayItemData {
    item: StatusNotifierItem,
    menu: Option<TrayMenu>,
}

enum TrayCommand {
    Activate(ActivateRequest),
    AboutToShow {
        address: String,
        menu_path: String,
        id: i32,
    },
}

type TrayCmdSender = tokio::sync::mpsc::UnboundedSender<TrayCommand>;

fn send_tray_snapshot(
    tx: &std::sync::mpsc::Sender<HashMap<String, TrayItemData>>,
    client: &TrayClient,
) {
    let items = client.items();
    let snapshot: HashMap<String, TrayItemData> = {
        let guard = match items.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard
            .iter()
            .map(|(address, (item, menu))| {
                (
                    address.clone(),
                    TrayItemData {
                        item: item.clone(),
                        menu: menu.clone(),
                    },
                )
            })
            .collect()
    };
    let _ = tx.send(snapshot);
}

fn spawn_tray_watcher() -> (
    std::sync::mpsc::Receiver<HashMap<String, TrayItemData>>,
    TrayCmdSender,
) {
    let (tx, rx) = std::sync::mpsc::channel::<HashMap<String, TrayItemData>>();
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<TrayCommand>();

    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                eprintln!("[altdock] failed to start tray runtime: {e}");
                return;
            }
        };

        rt.block_on(async move {
            let client = match TrayClient::new().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[altdock] failed to start tray client: {e}");
                    return;
                }
            };

            let mut events = client.subscribe();
            send_tray_snapshot(&tx, &client);

            loop {
                tokio::select! {
                    ev = events.recv() => {
                        match ev {
                            Ok(_event) => send_tray_snapshot(&tx, &client),
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                send_tray_snapshot(&tx, &client);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(TrayCommand::Activate(req)) => {
                                if let Err(e) = client.activate(req).await {
                                    eprintln!("[altdock] tray activate failed: {e}");
                                }
                            }
                            Some(TrayCommand::AboutToShow { address, menu_path, id }) => {
                                let _ = client.about_to_show_menuitem(address, menu_path, id).await;
                            }
                            None => break,
                        }
                    }
                }
            }
        });
    });

    (rx, cmd_tx)
}

fn tray_icon_image(item: &StatusNotifierItem) -> Image {
    let icon_name = item
        .icon_name
        .clone()
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "image-missing".to_string());
    let icon = Image::from_icon_name(&icon_name);
    icon.set_pixel_size(32);
    icon
}

fn tray_item_tooltip(item: &StatusNotifierItem) -> String {
    item.title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| item.tool_tip.as_ref().map(|t| t.title.clone()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| item.id.clone())
}

struct TrayMenuLayer {
    container: GtkBox,
    open_address: RefCell<Option<String>>,
    hidden_anchor: RefCell<Option<Button>>,
}

impl TrayMenuLayer {
    fn new() -> Rc<Self> {
        let container = GtkBox::new(gtk4::Orientation::Vertical, 0);
        container.add_css_class("trayMenuLayer");
        Rc::new(Self {
            container,
            open_address: RefCell::new(None),
            hidden_anchor: RefCell::new(None),
        })
    }

    fn close(&self) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        *self.open_address.borrow_mut() = None;
        let anchor = self.hidden_anchor.borrow_mut().take();
        if let Some(btn) = anchor {
            btn.set_visible(true);
        }
    }

    fn close_and_resize(&self) {
        self.close();
        glib::idle_add_local_once(reset_window_size);
    }

    fn is_open_for(&self, address: &str) -> bool {
        self.open_address.borrow().as_deref() == Some(address)
    }
}

fn build_menu_box(
    items: &[TrayMenuItem],
    address: &str,
    menu_path: &str,
    cmd_tx: &TrayCmdSender,
    layer: &Rc<TrayMenuLayer>,
) -> GtkBox {
    let menu_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
    menu_box.set_halign(gtk4::Align::End);
    menu_box.set_hexpand(true);
    menu_box.add_css_class("trayMenuBox");

    for item in items {
        if !item.visible {
            continue;
        }

        if matches!(item.menu_type, TrayMenuType::Separator) {
            let sep = Separator::new(gtk4::Orientation::Horizontal);
            sep.add_css_class("trayMenuSeparator");
            sep.set_margin_start(10);
            sep.set_margin_end(10);
            menu_box.append(&sep);
            continue;
        }

        let raw_label = item.label.clone().unwrap_or_default();
        let mut label_text = String::with_capacity(raw_label.len());
        let mut chars = raw_label.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '_' {
                if chars.peek() == Some(&'_') {
                    chars.next();
                    label_text.push('_');
                }
            } else {
                label_text.push(c);
            }
        }
        if item.toggle_state == TrayToggleState::On {
            label_text = format!("{label_text}");
        }

        let label = gtk4::Label::builder().label(&label_text).xalign(0.0).build();
        let row = Button::builder().css_classes(["trayMenuItem"]).build();
        row.set_child(Some(&label));
        row.set_sensitive(item.enabled);

        if !item.submenu.is_empty() {
            let submenu_items = item.submenu.clone();
            let address = address.to_string();
            let menu_path = menu_path.to_string();
            let cmd_tx = cmd_tx.clone();
            let layer = layer.clone();
            let item_id = item.id;

            row.connect_clicked(move |btn| {
                let _ = cmd_tx.send(TrayCommand::AboutToShow {
                    address: address.clone(),
                    menu_path: menu_path.clone(),
                    id: item_id,
                });

                let submenu_box = build_menu_box(
                    &submenu_items,
                    &address,
                    &menu_path,
                    &cmd_tx,
                    &layer,
                );

                let child_popover = Popover::builder()
                    .child(&submenu_box)
                    .position(gtk4::PositionType::Right)
                    .has_arrow(true)
                    .build();
                child_popover.set_parent(btn);
                child_popover.popup();
            });
        } else {
            let address = address.to_string();
            let menu_path = menu_path.to_string();
            let cmd_tx = cmd_tx.clone();
            let layer = layer.clone();
            let item_id = item.id;

            row.connect_clicked(move |_| {
                let _ = cmd_tx.send(TrayCommand::Activate(ActivateRequest::MenuItem {
                    address: address.clone(),
                    menu_path: menu_path.clone(),
                    submenu_id: item_id,
                }));
                layer.close_and_resize();
            });
        }

        menu_box.append(&row);
    }

    menu_box
}

fn show_tray_menu(
    anchor: &Button,
    layer: &Rc<TrayMenuLayer>,
    address: &str,
    menu: &TrayMenu,
    menu_path: &str,
    cmd_tx: &TrayCmdSender,
) {
    close_dock_menu();

    if layer.is_open_for(address) {
        layer.close();
        return;
    }

    layer.close();

    let _ = cmd_tx.send(TrayCommand::AboutToShow {
        address: address.to_string(),
        menu_path: menu_path.to_string(),
        id: 0,
    });

    let popover = GtkBox::builder()
        .css_classes(["trayMenuPopover"])
        .build();

    let menu_box = build_menu_box(&menu.submenus, address, menu_path, cmd_tx, layer);

    let back_label = gtk4::Label::builder().label("← Back").xalign(0.0).build();
    let back = Button::builder().css_classes(["trayMenuItem"]).build();
    back.set_child(Some(&back_label));
    {
        let layer = layer.clone();
        back.connect_clicked(move |_| layer.close_and_resize());
    }
    menu_box.prepend(&back);

    let close_gesture = GestureClick::new();
    close_gesture.set_button(gdk::BUTTON_SECONDARY);
    close_gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let layer = layer.clone();
        close_gesture.connect_released(move |_, _, _, _| {
            let layer = layer.clone();
            glib::idle_add_local_once(move || layer.close_and_resize());
        });
    }
    menu_box.add_controller(close_gesture);

    popover.append(&menu_box);
    layer.container.append(&popover);
    popover.set_visible(true);

    anchor.set_visible(false);
    *layer.hidden_anchor.borrow_mut() = Some(anchor.clone());
    *layer.open_address.borrow_mut() = Some(address.to_string());
}

fn make_tray_button(
    address: String,
    layer: &Rc<TrayMenuLayer>,
    data_cell: Rc<RefCell<TrayItemData>>,
    cmd_tx: TrayCmdSender,
) -> Button {
    let (icon, tooltip) = {
        let data = data_cell.borrow();
        (tray_icon_image(&data.item), tray_item_tooltip(&data.item))
    };

    let btn = Button::builder()
        .child(&icon)
        .css_classes(["trayBtn"])
        .tooltip_text(&tooltip)
        .vexpand(true)
        .valign(gtk4::Align::Start)
        .height_request(80)
        .build();

    let click_gesture = GestureClick::new();
    click_gesture.set_button(0);

    let addr = address.clone();
    let btn_weak = btn.downgrade();
    let layer = layer.clone();

    click_gesture.connect_released(move |gesture, _n_press, x, y| {
        let Some(btn) = btn_weak.upgrade() else {
            return;
        };
        let button = gesture.current_button();

        let (item_is_menu, menu_path, menu) = {
            let data = data_cell.borrow();
            (
                data.item.item_is_menu,
                data.item.menu.clone().unwrap_or_default(),
                data.menu.clone(),
            )
        };

        match button {
            gdk::BUTTON_SECONDARY => {
                if let Some(menu) = &menu {
                    show_tray_menu(&btn, &layer, &addr, menu, &menu_path, &cmd_tx);
                }
            }
            gdk::BUTTON_MIDDLE => {
                let _ = cmd_tx.send(TrayCommand::Activate(ActivateRequest::Secondary {
                    address: addr.clone(),
                    x: x as i32,
                    y: y as i32,
                }));
            }
            _ => {
                if item_is_menu {
                    if let Some(menu) = &menu {
                        show_tray_menu(&btn, &layer, &addr, menu, &menu_path, &cmd_tx);
                        return;
                    }
                }
                let _ = cmd_tx.send(TrayCommand::Activate(ActivateRequest::Default {
                    address: addr.clone(),
                    x: x as i32,
                    y: y as i32,
                }));
            }
        }
    });

    btn.add_controller(click_gesture);
    btn
}

#[derive(Default)]
struct TrayState {
    addresses: Vec<String>,
    buttons: HashMap<String, Button>,
    data: HashMap<String, Rc<RefCell<TrayItemData>>>,
}

fn update_traybox(
    tray_box: &GtkBox,
    state: &Rc<RefCell<TrayState>>,
    items: &HashMap<String, TrayItemData>,
    cmd_tx: &TrayCmdSender,
    layer: &Rc<TrayMenuLayer>,
) {
    let mut state = state.borrow_mut();

    let mut incoming: Vec<String> = items.keys().cloned().collect();
    incoming.sort();

    if incoming == state.addresses {
        for (address, data) in items {
            if let Some(btn) = state.buttons.get(address) {
                btn.set_child(Some(&tray_icon_image(&data.item)));
                btn.set_tooltip_text(Some(&tray_item_tooltip(&data.item)));
            }
            if let Some(cell) = state.data.get(address) {
                *cell.borrow_mut() = data.clone();
            }
        }
        return;
    }

    layer.close();

    while let Some(child) = tray_box.first_child() {
        tray_box.remove(&child);
    }
    state.buttons.clear();
    state.data.clear();

    for address in &incoming {
        let data = items[address].clone();
        let data_cell = Rc::new(RefCell::new(data));
        let btn = make_tray_button(address.clone(), layer, data_cell.clone(), cmd_tx.clone());
        tray_box.append(&btn);
        state.buttons.insert(address.clone(), btn);
        state.data.insert(address.clone(), data_cell);
    }
    state.addresses = incoming;
}

thread_local! { 
    static OPEN_DOCK_MENU: RefCell<Option<(u64, GtkBox, Button)>> = const { RefCell::new(None) };
}

thread_local! {
    static DOCK_WINDOW: RefCell<Option<ApplicationWindow>> = const { RefCell::new(None) };
}

fn reset_window_size() {
    let win = DOCK_WINDOW.with(|w| w.borrow().clone());
    if let Some(win) = win {
        win.set_visible(false);
        win.set_visible(true);
    }
}

fn close_dock_menu_and_resize() {
    close_dock_menu();
    glib::idle_add_local_once(reset_window_size);
}

fn close_dock_menu() {
    let taken = OPEN_DOCK_MENU.with(|m| m.borrow_mut().take());
    if let Some((_, slot, anchor)) = taken {
        while let Some(child) = slot.first_child() {
            slot.remove(&child);
        }
        anchor.set_visible(true);
    }
}

fn niri_window_pid(id: u64) -> Option<i32> {
    let mut sock = Socket::connect().ok()?;
    match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(ws))) => ws.into_iter().find(|w| w.id == id).and_then(|w| w.pid),
        _ => None,
    }
}

fn force_quit_window(id: u64) {
    let Ok(mut sock) = Socket::connect() else {
        eprintln!("[altdock] force quit: cannot connect to niri");
        return;
    };
    let windows = match sock.send(Request::Windows) {
        Ok(Ok(Response::Windows(ws))) => ws,
        _ => {
            eprintln!("[altdock] force quit: could not list windows");
            return;
        }
    };
    let Some(pid) = windows.iter().find(|w| w.id == id).and_then(|w| w.pid) else {
        eprintln!("[altdock] force quit: no pid for window {id}");
        return;
    };

    let shared = windows.iter().any(|w| w.id != id && w.pid == Some(pid));

    if shared {
        eprintln!(
            "[altdock] force quit: pid {pid} also owns other windows, closing only window {id}"
        );
        send_action(Action::CloseWindow { id: Some(id) });
        return;
    }

    if let Err(e) = Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .spawn()
    {
        eprintln!("[altdock] force quit: failed to kill {pid}: {e}");
    }
}

fn desktop_dir() -> Option<std::path::PathBuf> {
    glib::user_special_dir(glib::UserDirectory::Desktop)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join("Desktop")))
}

fn exec_line_for_pid(pid: i32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|a| !a.is_empty())
        .map(|a| String::from_utf8_lossy(a).to_string())
        .collect();
    if args.is_empty() {
        return None;
    }
    let quoted: Vec<String> = args
        .iter()
        .map(|a| {
            let a = a.replace('%', "%%");
            if a.chars().any(|c| c.is_whitespace() || "\"'\\$`<>~|&;*?#()".contains(c)) {
                format!("\"{}\"", a.replace('\\', "\\\\").replace('"', "\\\""))
            } else {
                a
            }
        })
        .collect();
    Some(quoted.join(" "))
}

fn add_to_desktop(id: u64, app_id: &str, title: &str) {
    use std::os::unix::fs::PermissionsExt;

    let Some(dir) = desktop_dir() else {
        eprintln!("[altdock] add to desktop: could not find Desktop directory");
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[altdock] add to desktop: cannot create {}: {e}", dir.display());
        return;
    }

    let file_name = if app_id.is_empty() {
        format!("window-{id}.desktop")
    } else {
        format!("{app_id}.desktop")
    };
    let dest = dir.join(file_name);

    let result: std::io::Result<()> = if let Some(src) = desktop_for_app_id(app_id) {
        std::fs::copy(&src, &dest).map(|_| ())
    } else {
        let Some(exec) = niri_window_pid(id).and_then(exec_line_for_pid) else {
            eprintln!("[altdock] add to desktop: no .desktop file or command line for {app_id}");
            return;
        };
        let name = if title.is_empty() { app_id } else { title };
        let content = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nExec={exec}\nIcon={app_id}\nTerminal=false\n"
        );
        std::fs::write(&dest, content)
    };

    match result {
        Ok(()) => {
            let _ = std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755));
        }
        Err(e) => eprintln!("[altdock] add to desktop failed: {e}"),
    }
}

fn is_window_tiled(id: u64) -> bool {
    let Ok(mut sock) = Socket::connect() else {
        return false;
    };
    let Ok(Ok(Response::Windows(windows))) = sock.send(Request::Windows) else {
        return false;
    };
    let Some(me) = windows.iter().find(|w| w.id == id) else {
        return false;
    };
    if me.is_floating {
        return false;
    }
    let has_sibling = windows
        .iter()
        .any(|o| o.id != id && !o.is_floating && o.workspace_id == me.workspace_id);
    has_sibling || TILED_MARK.with(|m| m.borrow().contains(&id))
}

fn remove_from_tiling(id: u64) {
    let origin = SPLIT_ORIGINS.with(|m| m.borrow_mut().remove(&id));

    if let Some(Some(origin_ws)) = origin {
        if let Ok(mut sock) = Socket::connect() {
            if let Ok(Ok(Response::Workspaces(workspaces))) = sock.send(Request::Workspaces) {
                if let Some(ws) = workspaces.iter().find(|w| w.id == origin_ws) {
                    move_window_to_ws(id, ws.id, ws.idx);
                } else {
                    eprintln!("[altdock] original workspace {origin_ws} no longer exists");
                }
            }
        }
    }

    send_action_checked(
        "move-window-to-floating",
        Action::MoveWindowToFloating { id: Some(id) },
    );
    send_action(Action::FocusWindow { id });
}

fn make_menu_item(label: &str, enabled: bool, on_click: impl Fn() + 'static) -> Button {
    let lbl = gtk4::Label::builder().label(label).xalign(0.0).build();
    let item = Button::builder()
        .css_classes(["trayMenuItem", "dockMenuItem"])
        .build();
    item.set_child(Some(&lbl));
    item.set_sensitive(enabled);
    item.connect_clicked(move |_| {
        close_dock_menu_and_resize();
        on_click();
    });
    item
}

fn show_dock_menu(anchor: &Button, id: u64, app_id: &str) {
    let title = anchor
        .tooltip_text()
        .map(|t| t.to_string())
        .unwrap_or_default();

    let same_icon = OPEN_DOCK_MENU.with(|m| {
        m.borrow()
            .as_ref()
            .map(|(open_id, _, _)| *open_id == id)
            .unwrap_or(false)
    });
    close_dock_menu();
    if same_icon {
        return;
    }

    let Some(wrapper) = anchor.parent().and_then(|p| p.downcast::<GtkBox>().ok()) else {
        return;
    };
    let Some(slot) = wrapper.last_child().and_then(|c| c.downcast::<GtkBox>().ok()) else {
        return;
    };

    let menu_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
    menu_box.set_halign(gtk4::Align::Start);
    menu_box.add_css_class("trayMenuBox");

    menu_box.append(&make_menu_item("← Back", true, || {}));

    let close_gesture = GestureClick::new();
    close_gesture.set_button(gdk::BUTTON_SECONDARY);
    close_gesture.set_propagation_phase(gtk4::PropagationPhase::Capture);
    close_gesture.connect_released(|_, _, _, _| {
        glib::idle_add_local_once(close_dock_menu_and_resize);
    });
    menu_box.add_controller(close_gesture);

    {
        let app_id = app_id.to_string();
        menu_box.append(&make_menu_item("Add to desktop", true, move || {
            add_to_desktop(id, &app_id, &title);
        }));
    }
    menu_box.append(&make_menu_item("Remove from Snap", is_window_tiled(id), move || {
        remove_from_tiling(id)
    }));
    menu_box.append(&make_menu_item("Quit Application", true, move || {
        force_quit_window(id);
    }));

    let popover = GtkBox::builder().css_classes(["trayMenuPopover"]).build();
    popover.append(&menu_box);
    slot.append(&popover);
    popover.set_visible(true);

    anchor.set_visible(false);
    OPEN_DOCK_MENU.with(|m| *m.borrow_mut() = Some((id, slot, anchor.clone())));
}

fn make_dock_btn(win: &NiriWindow) -> Button {
    let icon_name = if win.app_id.is_empty() {
        "application-x-executable".to_string()
    } else {
        win.app_id.clone()
    };

    let icon = Image::from_icon_name(&icon_name);
    icon.set_pixel_size(62);
    icon.set_halign(gtk4::Align::Start);

    let label_text = if win.title.is_empty() {
        win.app_id.clone()
    } else {
        win.title.clone()
    };

    let btn = Button::builder()
        .child(&icon)
        .css_classes(["dockBtn"])
        .tooltip_text(&label_text)
        .vexpand(false)
        .valign(gtk4::Align::Start)
        .halign(gtk4::Align::Center)
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

    let hint_ctrl = gtk4::DropControllerMotion::new();
    {
        let hint_btn = btn.downgrade();
        hint_ctrl.connect_enter(move |_, _, _| {
            if let Some(b) = hint_btn.upgrade() {
                let dragged = DRAGGED_WINDOW.with(|d| d.get());
                if dragged.is_some() && dragged != Some(id) {
                    b.add_css_class("dockBtnHint");
                }
            }
        });
    }
    {
        let hint_btn = btn.downgrade();
        hint_ctrl.connect_leave(move |_| {
            if let Some(b) = hint_btn.upgrade() {
                b.remove_css_class("dockBtnHint");
            }
        });
    }
    btn.add_controller(hint_ctrl);

    let drop_btn = btn.downgrade();
    let drop_target = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    drop_target.connect_drop(move |_, _, _, _| {
        if let Some(b) = drop_btn.upgrade() {
            b.remove_css_class("dockBtnHint");
        }
        let Some(dragged) = DRAGGED_WINDOW.with(|d| d.take()) else {
            return false;
        };
        if dragged == id {
            return false;
        }
        if DESKTOP_SHOWN_BY_DRAG.with(|d| d.take()) {
            toggle_show_desktop();
        }
        tile_pair_in_new_workspace(id, dragged);
        true
    });
    btn.add_controller(drop_target);

    let restore_gesture = GestureClick::new();
    restore_gesture.set_button(gdk::BUTTON_MIDDLE);
    restore_gesture.connect_released(move |_, _, _, _| restore_split(id));
    btn.add_controller(restore_gesture);

    let menu_gesture = GestureClick::new();
    menu_gesture.set_button(gdk::BUTTON_SECONDARY);
    let menu_btn = btn.downgrade();
    let menu_app_id = win.app_id.clone();
    menu_gesture.connect_released(move |_, _, _, _| {
        let Some(btn) = menu_btn.upgrade() else {
            return;
        };
        show_dock_menu(&btn, id, &menu_app_id);
    });
    btn.add_controller(menu_gesture);

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

        drag_source.connect_drag_end(move |_, _, _| {
            glib::idle_add_local_once(|| {
                DRAGGED_WINDOW.with(|d| d.set(None));
                DESKTOP_SHOWN_BY_DRAG.with(|d| d.set(false));
            });
        });

        drag_source.connect_drag_begin(move |src, _drag| {
            DRAGGED_WINDOW.with(|d| d.set(Some(id)));
            if let Some(paintable) = drag_icon.paintable() {
                src.set_icon(Some(&paintable), 0, 0);
            }
        });

        icon.add_controller(drag_source);
    }

    btn
}

thread_local! {
    static TILED_MARK: RefCell<std::collections::HashSet<u64>> =
        RefCell::new(std::collections::HashSet::new());
}

fn tiled_window_ids(windows: &[NiriWindow]) -> Vec<u64> {
    let tiled: Vec<&NiriWindow> = windows
        .iter()
        .filter(|w| !w.is_floating && w.workspace_id.is_some())
        .collect();
    let by_rule: Vec<u64> = tiled
        .iter()
        .filter(|w| {
            tiled
                .iter()
                .any(|o| o.id != w.id && o.workspace_id == w.workspace_id)
        })
        .map(|w| w.id)
        .collect();

    TILED_MARK.with(|m| {
        let mut m = m.borrow_mut();
        m.extend(by_rule);
        if !windows.is_empty() {
            m.retain(|id| windows.iter().any(|w| w.id == *id && !w.is_floating));
        }
        windows
            .iter()
            .filter(|w| m.contains(&w.id))
            .map(|w| w.id)
            .collect()
    })
}

fn apply_dock_btn_classes(btn: &Button, win: &NiriWindow, tiled: bool) {
    if win.is_focused {
        btn.add_css_class("dockBtnActive");
    } else {
        btn.remove_css_class("dockBtnActive");
    }
    if tiled {
        btn.add_css_class("dockBtnTiled");
    } else {
        btn.remove_css_class("dockBtnTiled");
    }
}

#[derive(Default)]
struct DockState {
    buttons: HashMap<u64, Button>,
    order: Vec<u64>,
}

fn clear_dock_hints(state: &Rc<RefCell<DockState>>) {
    if let Ok(st) = state.try_borrow() {
        for btn in st.buttons.values() {
            btn.remove_css_class("dockBtnHint");
        }
    }
}

fn update_dockbox(
    dockbox: &GtkBox,
    tray_box: &GtkBox,
    tray_menu_layer: &GtkBox,
    state: &Rc<RefCell<DockState>>,
    windows: &[NiriWindow],
) {
    let mut state = state.borrow_mut();
    let new_order: Vec<u64> = windows.iter().map(|w| w.id).collect();
    let tiled = tiled_window_ids(windows);

    if !windows.is_empty() && new_order == state.order {
        for win in windows {
            if let Some(btn) = state.buttons.get(&win.id) {
                apply_dock_btn_classes(btn, win, tiled.contains(&win.id));
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

    close_dock_menu();
    while let Some(child) = dockbox.first_child() {
        dockbox.remove(&child);
    }
    state.buttons.clear();

    if windows.is_empty() {
        let empty = gtk4::Label::new(Some("No open apps"));
        empty.add_css_class("dockEmpty");
        dockbox.append(&empty);
        state.order.clear();
        dockbox.append(tray_box);
        dockbox.append(tray_menu_layer);
        return;
    }

    for win in windows {
        let btn = make_dock_btn(win);
        apply_dock_btn_classes(&btn, win, tiled.contains(&win.id));

        let wrapper = GtkBox::new(gtk4::Orientation::Vertical, 0);
        wrapper.set_valign(gtk4::Align::Start);
        wrapper.append(&btn);
        let slot = GtkBox::new(gtk4::Orientation::Vertical, 0);
        slot.add_css_class("dockMenuSlot");
        wrapper.append(&slot);

        dockbox.append(&wrapper);
        state.buttons.insert(win.id, btn);
    }
    state.order = new_order;

    let tray_main = GtkBox::builder()
        .spacing(5)
        .orientation(gtk4::Orientation::Vertical)
        .build();

    tray_main.append(tray_box);
    tray_main.append(tray_menu_layer);

    // dockbox.append(tray_box);
    dockbox.append(&tray_main);
}

pub fn spawn_altdock(app: &Application, dockbox: GtkBox) -> ApplicationWindow {
    let win = ApplicationWindow::builder()
        .application(app)
        .title("AltDock")
        .css_classes(["dockOverlay"])
        .build();

    win.init_layer_shell();
    win.set_namespace(Some("AltDock"));
    win.set_layer(Layer::Overlay);
    win.remove_css_class("background");
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, false);
    win.set_anchor(Edge::Right, false);
    win.set_anchor(Edge::Bottom, false);

    let dockbox_clone_anim = dockbox.clone();

    let dockbox_rc = Rc::new(dockbox);
    let dock_state: Rc<RefCell<DockState>> = Rc::new(RefCell::new(DockState::default()));

    let tray_box = GtkBox::new(gtk4::Orientation::Horizontal, 4);
    tray_box.add_css_class("tray");
    tray_box.set_hexpand(true);
    tray_box.set_halign(gtk4::Align::End);

    let tray_state: Rc<RefCell<TrayState>> = Rc::new(RefCell::new(TrayState::default()));
    let tray_menu_layer = TrayMenuLayer::new();

    let initial = windows_sorted(&get_niri_windows_map());
    update_dockbox(
        &dockbox_rc,
        &tray_box,
        &tray_menu_layer.container,
        &dock_state,
        &initial,
    );

    let niri_rx = Rc::new(RefCell::new(spawn_niri_watcher()));

    {
        let dockbox_rc = dockbox_rc.clone();
        let tray_box = tray_box.clone();
        let tray_menu_layer = tray_menu_layer.clone();
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
                update_dockbox(
                    &dockbox_rc,
                    &tray_box,
                    &tray_menu_layer.container,
                    &dock_state,
                    &windows,
                );
            }

            glib::ControlFlow::Continue
        });
    }

    let (tray_rx, tray_cmd_tx) = spawn_tray_watcher();
    let tray_rx = Rc::new(RefCell::new(tray_rx));

    {
        let tray_box = tray_box.clone();
        let tray_state = tray_state.clone();
        let tray_rx = tray_rx.clone();
        let tray_cmd_tx = tray_cmd_tx.clone();
        let tray_menu_layer = tray_menu_layer.clone();

        glib::timeout_add_local(Duration::from_millis(200), move || {
            let rx = tray_rx.borrow();
            let mut latest: Option<HashMap<String, TrayItemData>> = None;
            while let Ok(items) = rx.try_recv() {
                latest = Some(items);
            }
            drop(rx);

            if let Some(items) = latest {
                update_traybox(&tray_box, &tray_state, &items, &tray_cmd_tx, &tray_menu_layer);
            }

            glib::ControlFlow::Continue
        });
    }

    let hover_ctrl = gtk4::EventControllerMotion::new();
    let pending: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let pending_leavein = Rc::clone(&pending);
    let pop_leave = dockbox_rc.clone();
    let win_size_setter = win.clone();
    let leave_tray_menu_layer = tray_menu_layer.clone();
    let leave_dock_state = dock_state.clone();
    hover_ctrl.connect_leave(move |_| {
        pending_leavein.set(true);
        let pop = pop_leave.clone();
        let win = win_size_setter.clone();
        let pending_hide = Rc::clone(&pending_leavein);
        let dockbox_clone_anim = dockbox_clone_anim.clone();
        let tray_menu_layer = leave_tray_menu_layer.clone();
        let dock_state = leave_dock_state.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if pending_hide.get() {
                dockbox_clone_anim.remove_css_class("dockcum");
                dockbox_clone_anim.add_css_class("dockleave");

                let pop2 = pop.clone();
                let win2 = win.clone();
                let pending_hide2 = Rc::clone(&pending_hide);
                let tray_menu_layer = tray_menu_layer.clone();
                let dock_state = dock_state.clone();
                glib::timeout_add_local(Duration::from_millis(300), move || {
                    if pending_hide2.get() {
                        clear_dock_hints(&dock_state);
                        pop2.set_visible(false);
                        win2.set_visible(false);
                        win2.set_visible(true);
                        tray_menu_layer.close();
                        close_dock_menu();
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

    win.add_controller(hover_ctrl);

    let drag_motion = gtk4::DropControllerMotion::new();
    drag_motion.connect_leave(|_| {
        if DRAGGED_WINDOW.with(|d| d.get()).is_none() {
            return;
        }
        glib::timeout_add_local_once(Duration::from_millis(80), || {
            if DRAGGED_WINDOW.with(|d| d.get()).is_some()
                && !DESKTOP_SHOWN_BY_DRAG.with(|d| d.get())
                && !cbq_ifdesktop()
            {
                DESKTOP_SHOWN_BY_DRAG.with(|d| d.set(true));
                toggle_show_desktop();
            }
        });
    });
    drag_motion.connect_enter(|_, _, _| {
        if DRAGGED_WINDOW.with(|d| d.get()).is_some()
            && DESKTOP_SHOWN_BY_DRAG.with(|d| d.take())
        {
            toggle_show_desktop();
        }
    });
    win.add_controller(drag_motion);

    DOCK_WINDOW.with(|w| *w.borrow_mut() = Some(win.clone()));

    win.present();
    // win.set_visible(false);
    win
}