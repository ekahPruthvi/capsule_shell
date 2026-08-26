use gtk4::gdk;
use gtk4::glib;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, Button, DragSource, EventControllerMotion,
    GestureClick, Image, Popover, Separator, prelude::*,
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

use system_tray::client::{ActivateRequest, Client as TrayClient};
use system_tray::item::StatusNotifierItem;
use system_tray::menu::{
    MenuItem as TrayMenuItem, MenuType as TrayMenuType, ToggleState as TrayToggleState, TrayMenu,
};

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

pub fn spawn_tray_watcher() -> (
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
    icon.set_pixel_size(22);
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
}

impl TrayMenuLayer {
    fn new() -> Rc<Self> {
        let container = GtkBox::new(gtk4::Orientation::Vertical, 0);
        container.add_css_class("trayMenuLayer");
        Rc::new(Self {
            container,
            open_address: RefCell::new(None),
        })
    }

    /// Close whatever tray menu is currently open (if any).
    fn close(&self) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        *self.open_address.borrow_mut() = None;
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
    menu_box.add_css_class("trayMenuBox");

    for item in items {
        if !item.visible {
            continue;
        }

        if matches!(item.menu_type, TrayMenuType::Separator) {
            let sep = Separator::new(gtk4::Orientation::Horizontal);
            sep.add_css_class("trayMenuSeparator");
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
                layer.close();
            });
        }

        menu_box.append(&row);
    }

    menu_box
}

fn show_tray_menu(
    _anchor: &Button,
    layer: &Rc<TrayMenuLayer>,
    address: &str,
    menu: &TrayMenu,
    menu_path: &str,
    cmd_tx: &TrayCmdSender,
) {
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
    popover.append(&menu_box);
    layer.container.append(&popover);
    popover.set_visible(true);

    *layer.open_address.borrow_mut() = Some(address.to_string());
}

fn make_tray_button(
    address: String,
    layer: &Rc<TrayMenuLayer>,
    data: &TrayItemData,
    cmd_tx: TrayCmdSender,
) -> Button {
    let icon = tray_icon_image(&data.item);
    let tooltip = tray_item_tooltip(&data.item);

    let btn = Button::builder()
        .child(&icon)
        .css_classes(["trayBtn"])
        .tooltip_text(&tooltip)
        .vexpand(true)
        .valign(gtk4::Align::Fill)
        .build();

    let item_is_menu = data.item.item_is_menu;
    let menu_path = data.item.menu.clone().unwrap_or_default();
    let menu = data.menu.clone();

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
        }
        return;
    }

    layer.close();

    while let Some(child) = tray_box.first_child() {
        tray_box.remove(&child);
    }
    state.buttons.clear();

    for address in &incoming {
        let data = &items[address];
        let btn = make_tray_button(address.clone(), layer, data, cmd_tx.clone());
        tray_box.append(&btn);
        state.buttons.insert(address.clone(), btn);
    }
    state.addresses = incoming;
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
        .vexpand(false)
        .valign(gtk4::Align::Start)
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

fn update_dockbox(
    dockbox: &GtkBox,
    tray_box: &GtkBox,
    tray_menu_layer: &GtkBox,
    state: &Rc<RefCell<DockState>>,
    windows: &[NiriWindow],
) {
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
        dockbox.append(tray_box);
        dockbox.append(tray_menu_layer);
        return;
    }

    for win in windows {
        let btn = make_dock_btn(win);
        dockbox.append(&btn);
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

    win.set_child(Some(&*dockbox_rc));

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
    let pop_leave = win.clone();
    let leave_tray_menu_layer = tray_menu_layer.clone();
    hover_ctrl.connect_leave(move |_| {
        pending_leavein.set(true);
        let pop = pop_leave.clone();
        let pending_hide = Rc::clone(&pending_leavein);
        let dockbox_clone_anim = dockbox_clone_anim.clone();
        let tray_menu_layer = leave_tray_menu_layer.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            if pending_hide.get() {
                dockbox_clone_anim.remove_css_class("dockcum");
                dockbox_clone_anim.add_css_class("dockleave");

                let pop2 = pop.clone();
                let pending_hide2 = Rc::clone(&pending_hide);
                let tray_menu_layer = tray_menu_layer.clone();
                glib::timeout_add_local(Duration::from_millis(400), move || {
                    if pending_hide2.get() {
                        pop2.set_visible(false);
                        tray_menu_layer.close();
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