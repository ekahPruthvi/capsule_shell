use gtk4::{
    Application, ApplicationWindow, Label, Box as GtkBox, Button, Orientation, prelude::*,
    DrawingArea, gdk_pixbuf::Pixbuf, Image, EventControllerScroll, EventControllerScrollFlags,
    Switch,
};
use gtk4::glib;
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::{
    time::Duration, 
    cell::RefCell,
    cell::Cell,
    rc::Rc,
    collections::HashMap,
    fs::File,
};
use std::io::{BufRead, BufReader, Write, BufWriter};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::TryRecvError;

#[derive(Debug, Clone, PartialEq)]
pub enum NetworkState {
    WifiConnected(String),       
    EthernetConnected(String),   
    NoInternet,                 
    Disconnected,
    WifiOff,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundState {
    pub volume:     u32,
    pub muted:      bool,
    pub sink:       String,
    pub mic_volume: u32,
    pub mic_muted:  bool,
}

fn get_volume_and_mute(target: &str) -> (u32, bool) {
    let out = std::process::Command::new("wpctl")
        .args(["get-volume", target])
        .output();

    if let Ok(out) = out {
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let is_muted = text.contains("[MUTED]");
        let vol = text
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| (v * 100.0).round() as u32)
            .unwrap_or(0);
        (vol, is_muted)
    } else {
        (0, false)
    }
}

fn get_sound_state() -> SoundState {
    let (volume, muted) = get_volume_and_mute("@DEFAULT_AUDIO_SINK@");
    let (mic_volume, mic_muted) = get_volume_and_mute("@DEFAULT_AUDIO_SOURCE@");

    let sink = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .and_then(|o| {
            let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if raw.is_empty() { return None; }

            let list = std::process::Command::new("pactl")
                .args(["list", "sinks"])
                .output()
                .ok()?;
            let list_text = String::from_utf8_lossy(&list.stdout).to_string();

            let mut in_sink  = false;
            let mut desc: Option<String> = None;
            for line in list_text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Name:") {
                    in_sink = trimmed.contains(&raw);
                }
                if in_sink {
                    if let Some(d) = trimmed.strip_prefix("Description:") {
                        desc = Some(d.trim().to_string());
                        break;
                    }
                }
            }
            desc.or(Some(raw))
        })
        .unwrap_or_else(|| "Unknown Output".to_string());

    SoundState { volume, muted, sink, mic_volume, mic_muted }
}

fn sound_icon(state: &SoundState) -> &'static str {
    if state.muted || state.volume == 0 {
        "/var/lib/cynager/icons/soundmute.svg"
    } else if state.volume <= 33 {
        "/var/lib/cynager/icons/soundlow.svg"
    } else if state.volume <= 66 {
        "/var/lib/cynager/icons/soundmed.svg"
    } else {
        "/var/lib/cynager/icons/soundhigh.svg"
    }
}

fn spawn_pactl_event_pump(keywords: &'static [&'static str]) -> std::sync::mpsc::Receiver<()> {
    let (evt_tx, evt_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        loop {
            let child = std::process::Command::new("pactl")
                .args(["subscribe"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn();

            let mut child = match child {
                Ok(c) => c,
                Err(_) => {
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = std::io::BufReader::new(stdout);
                for line in reader.lines() {
                    let line = match line {
                        Ok(l) => l,
                        Err(_) => break,
                    };
                    if keywords.iter().any(|k| line.contains(k)) {
                        if evt_tx.send(()).is_err() {
                            let _ = child.kill();
                            let _ = child.wait();
                            return;
                        }
                    }
                }
            }

            let _ = child.kill();
            let _ = child.wait();
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    evt_rx
}

fn wait_for_event_batch(evt_rx: &std::sync::mpsc::Receiver<()>, fallback: Duration, debounce: Duration) {
    match evt_rx.recv_timeout(fallback) {
        Ok(()) => {
            while evt_rx.recv_timeout(debounce).is_ok() {}
        }
        Err(_) => {
        }
    }
}

pub fn spawn_sound_watcher(fallback_interval: Duration) -> std::sync::mpsc::Receiver<SoundState> {
    let (tx, rx) = std::sync::mpsc::channel::<SoundState>();
    let evt_rx = spawn_pactl_event_pump(&["sink", "source", "server"]);

    std::thread::spawn(move || {
        let mut last = get_sound_state();
        if tx.send(last.clone()).is_err() { return; }

        loop {
            wait_for_event_batch(&evt_rx, fallback_interval, Duration::from_millis(40));

            let state = get_sound_state();
            if state != last {
                if tx.send(state.clone()).is_err() { break; }
                last = state;
            }
        }
    });
    rx
}


#[derive(Debug, Clone, PartialEq)]
pub struct OutputDevice {
    pub id:         String,  
    pub label:      String,  
    pub volume:     u32,
    pub muted:      bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppPlayback {
    pub id:        String,      
    pub label:     String,       
    pub volume:    u32,
    pub muted:     bool,
    pub icon_name: String,
}

fn parse_pactl_volume_pct(line: &str) -> u32 {
    if let Some(pct_pos) = line.find('%') {
        let bytes = line.as_bytes();
        let mut start = pct_pos;
        while start > 0 && bytes[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if let Ok(v) = line[start..pct_pos].trim().parse::<u32>() {
            return v;
        }
    }
    0
}

fn get_output_devices() -> Vec<OutputDevice> {
    let default_sink = std::process::Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    let out = match std::process::Command::new("pactl").args(["list", "sinks"]).output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();

    let mut devices = Vec::new();
    let mut name  = String::new();
    let mut desc  = String::new();
    let mut vol: u32 = 0;
    let mut muted = false;
    let mut in_block = false;

    macro_rules! flush {
        () => {
            if !name.is_empty() {
                let label = if desc.is_empty() { name.clone() } else { desc.clone() };
                devices.push(OutputDevice {
                    is_default: name == default_sink,
                    id: name.clone(),
                    label,
                    volume: vol,
                    muted,
                });
            }
            name.clear();
            desc.clear();
            vol = 0;
            muted = false;
        };
    }

    for line in text.lines() {
        if line.starts_with("Sink #") {
            if in_block { flush!(); }
            in_block = true;
            continue;
        }
        let t = line.trim();
        if let Some(v) = t.strip_prefix("Name:") {
            name = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("Description:") {
            desc = v.trim().to_string();
        } else if t.starts_with("Mute:") {
            muted = t.contains("yes");
        } else if t.starts_with("Volume:") {
            vol = parse_pactl_volume_pct(t);
        }
    }
    if in_block { flush!(); }

    devices
}

fn get_app_playbacks() -> Vec<AppPlayback> {
    let out = match std::process::Command::new("pactl").args(["list", "sink-inputs"]).output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();

    let mut apps = Vec::new();
    let mut id   = String::new();
    let mut vol: u32 = 0;
    let mut muted = false;
    let mut app_name: Option<String>   = None;
    let mut media_name: Option<String> = None;
    let mut icon_name: Option<String>  = None;
    let mut in_block = false;

    macro_rules! flush {
        () => {
            if !id.is_empty() {
                let label = app_name.clone()
                    .or_else(|| media_name.clone())
                    .unwrap_or_else(|| format!("Stream {}", id));
                let icon = icon_name.clone()
                    .or_else(|| app_name.clone().map(|n| n.to_lowercase().replace(' ', "-")))
                    .unwrap_or_else(|| "audio-x-generic-symbolic".to_string());
                apps.push(AppPlayback { id: id.clone(), label, volume: vol, muted, icon_name: icon });
            }
            id.clear();
            vol = 0;
            muted = false;
            app_name = None;
            media_name = None;
            icon_name = None;
        };
    }

    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Sink Input #") {
            if in_block { flush!(); }
            in_block = true;
            id = rest.trim().to_string();
            continue;
        }
        let t = line.trim();
        if t.starts_with("Mute:") {
            muted = t.contains("yes");
        } else if t.starts_with("Volume:") {
            vol = parse_pactl_volume_pct(t);
        } else if let Some(v) = t.strip_prefix("application.name = ") {
            app_name = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = t.strip_prefix("media.name = ") {
            media_name = Some(v.trim_matches('"').to_string());
        } else if let Some(v) = t.strip_prefix("application.icon_name = ") {
            icon_name = Some(v.trim_matches('"').to_string());
        }
    }
    if in_block { flush!(); }

    apps
}

fn set_output_volume(sink_name: &str, pct: u32) {
    let _ = std::process::Command::new("pactl")
        .args(["set-sink-volume", sink_name, &format!("{}%", pct)])
        .spawn();
}

fn set_default_sink(sink_name: &str) {
    let _ = std::process::Command::new("pactl")
        .args(["set-default-sink", sink_name])
        .spawn();
}

fn set_app_volume(sink_input_id: &str, pct: u32) {
    let _ = std::process::Command::new("pactl")
        .args(["set-sink-input-volume", sink_input_id, &format!("{}%", pct)])
        .spawn();
}

fn set_master_mute(mute: bool) {
    let val = if mute { "1" } else { "0" };
    let _ = std::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SINK@", val])
        .spawn();
}

fn set_mic_mute(mute: bool) {
    let val = if mute { "1" } else { "0" };
    let _ = std::process::Command::new("wpctl")
        .args(["set-mute", "@DEFAULT_AUDIO_SOURCE@", val])
        .spawn();
}

pub fn spawn_sound_devices_watcher(
    fallback_interval: Duration,
) -> std::sync::mpsc::Receiver<(Vec<OutputDevice>, Vec<AppPlayback>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    let evt_rx = spawn_pactl_event_pump(&["sink", "card"]);

    std::thread::spawn(move || {
        let mut last = (get_output_devices(), get_app_playbacks());
        if tx.send(last.clone()).is_err() { return; }

        loop {
            wait_for_event_batch(&evt_rx, fallback_interval, Duration::from_millis(40));

            let fresh = (get_output_devices(), get_app_playbacks());
            if fresh != last {
                if tx.send(fresh.clone()).is_err() { break; }
                last = fresh;
            }
        }
    });
    rx
}

struct SliderRowHandles {
    row:           gtk4::ListBoxRow,
    scale:         gtk4::Scale,
    handler:       glib::SignalHandlerId,
    value_lbl:     Label,
    name_lbl:      Label,
    last_touch:    Rc<Cell<std::time::Instant>>,
    radio:         Option<gtk4::CheckButton>,
    radio_handler: Option<glib::SignalHandlerId>,
}

#[derive(Clone)]
struct RadioConfig {
    group_leader: Rc<RefCell<Option<gtk4::CheckButton>>>,
    on_select:    Rc<dyn Fn(&str)>,
}

struct AppTileHandles {
    child:      gtk4::FlowBoxChild,
    value_lbl:  Label,
    name_lbl:   Label,
    volume:     Rc<Cell<u32>>,
    last_touch: Rc<Cell<std::time::Instant>>,
}

const APP_TILE_SCROLL_STEP: u32 = 5;

fn build_sound_row(
    id:         String,
    label_text: String,
    volume:     u32,
    is_default: bool,
    on_change:  Rc<dyn Fn(&str, u32)>,
    radio_cfg:  Option<RadioConfig>,
) -> SliderRowHandles {
    let root = GtkBox::new(Orientation::Vertical, 4);
    root.add_css_class("soundListRow");

    let top = GtkBox::new(Orientation::Horizontal, 8);

    let mut radio: Option<gtk4::CheckButton> = None;
    let mut radio_handler: Option<glib::SignalHandlerId> = None;

    if let Some(cfg) = radio_cfg {
        let check = gtk4::CheckButton::new();
        check.add_css_class("soundListRadio");
        check.set_valign(gtk4::Align::Center);
        check.set_tooltip_text(Some("Set as output device"));

        {
            let mut leader = cfg.group_leader.borrow_mut();
            if let Some(existing) = leader.as_ref() {
                check.set_group(Some(existing));
            } else {
                *leader = Some(check.clone());
            }
        }

        check.set_active(is_default);

        let id_for_radio = id.clone();
        let on_select = cfg.on_select.clone();
        let handler_id = check.connect_toggled(move |btn| {
            if btn.is_active() {
                on_select(&id_for_radio);
            }
        });

        top.append(&check);
        radio = Some(check);
        radio_handler = Some(handler_id);
    }

    let name_lbl = gtk4::Label::new(Some(&label_text));
    name_lbl.set_hexpand(true);
    name_lbl.set_halign(gtk4::Align::Start);
    name_lbl.add_css_class("soundListName");
    if is_default {
        name_lbl.add_css_class("soundListDefault");
    }

    let value_lbl = gtk4::Label::new(Some(&format!("{}%", volume)));
    value_lbl.add_css_class("soundListValue");

    top.append(&name_lbl);
    top.append(&value_lbl);

    let adjustment = gtk4::Adjustment::new(volume as f64, 0.0, 100.0, 1.0, 5.0, 0.0);
    let scale = gtk4::Scale::new(Orientation::Horizontal, Some(&adjustment));
    scale.set_draw_value(false);
    scale.set_hexpand(true);
    scale.add_css_class("soundListSlider");

    let last_touch = Rc::new(Cell::new(
        std::time::Instant::now() - Duration::from_secs(10),
    ));

    let id_for_cb     = id.clone();
    let last_touch_cb = last_touch.clone();
    let value_lbl_cb  = value_lbl.clone();
    let handler = scale.connect_value_changed(move |s| {
        let v = s.value().round() as u32;
        value_lbl_cb.set_label(&format!("{}%", v));
        last_touch_cb.set(std::time::Instant::now());
        on_change(&id_for_cb, v);
    });

    root.append(&top);
    top.append(&scale);

    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.set_child(Some(&root));
    row.add_css_class("soundListRowWrap");

    SliderRowHandles { row, scale, handler, value_lbl, name_lbl, last_touch, radio, radio_handler }
}

fn build_app_row(
    id:         String,
    label_text: String,
    icon_name:  String,
    volume:     u32,
    on_change:  Rc<dyn Fn(&str, u32)>,
) -> AppTileHandles {
    let tile = GtkBox::new(Orientation::Horizontal, 4);
    tile.add_css_class("appTile");
    tile.set_halign(gtk4::Align::Fill);
    tile.set_hexpand(true);
    tile.set_vexpand(false);
    tile.set_valign(gtk4::Align::Start);

    let name_n_val = GtkBox::new(Orientation::Vertical, 6);
    name_n_val.add_css_class("appTileTop");
    name_n_val.set_halign(gtk4::Align::End);
    name_n_val.set_hexpand(true);

    let icon = Image::from_icon_name(&icon_name);
    icon.set_pixel_size(28);
    icon.add_css_class("appTileIcon");

    let value_lbl = gtk4::Label::new(Some(&format!("{}%", volume)));
    value_lbl.add_css_class("soundApptxt");
    value_lbl.set_halign(gtk4::Align::End);
    value_lbl.set_justify(gtk4::Justification::Right);

    let name_lbl = gtk4::Label::new(Some(&label_text));
    name_lbl.add_css_class("soundApptxt");
    name_lbl.set_halign(gtk4::Align::End);
    name_lbl.set_justify(gtk4::Justification::Right);
    name_lbl.set_wrap(true);
    name_lbl.set_max_width_chars(100);


    name_n_val.append(&name_lbl);
    name_n_val.append(&value_lbl);

    tile.append(&icon);
    tile.append(&name_n_val);
    tile.set_tooltip_text(Some("Scroll to change volume"));

    let child = gtk4::FlowBoxChild::new();
    child.set_child(Some(&tile));
    child.set_focusable(false);
    child.add_css_class("appTileRow");

    let volume_cell = Rc::new(Cell::new(volume.min(100)));
    let last_touch  = Rc::new(Cell::new(
        std::time::Instant::now() - Duration::from_secs(10),
    ));

    let scroll = EventControllerScroll::new(
        EventControllerScrollFlags::VERTICAL | EventControllerScrollFlags::DISCRETE,
    );

    let id_for_scroll = id.clone();
    let volume_cell_for_scroll = volume_cell.clone();
    let last_touch_for_scroll = last_touch.clone();
    let value_lbl_for_scroll = value_lbl.clone();
    scroll.connect_scroll(move |_, _dx, dy| {
        let cur  = volume_cell_for_scroll.get() as i32;
        let step = APP_TILE_SCROLL_STEP as i32;
        let delta = if dy < 0.0 { step } else { -step };
        let new_vol = (cur + delta).clamp(0, 100) as u32;

        if new_vol != cur as u32 {
            volume_cell_for_scroll.set(new_vol);
            value_lbl_for_scroll.set_label(&format!("{}%", new_vol));
            last_touch_for_scroll.set(std::time::Instant::now());
            on_change(&id_for_scroll, new_vol);
        }

        glib::Propagation::Stop
    });
    child.add_controller(scroll);

    AppTileHandles { child, value_lbl, name_lbl, volume: volume_cell, last_touch }
}

fn ensure_placeholder(
    list_rc:     &Rc<gtk4::ListBox>,
    placeholder: &Rc<RefCell<Option<gtk4::ListBoxRow>>>,
    text:        &str,
) {
    if placeholder.borrow().is_none() {
        let lbl = gtk4::Label::new(Some(text));
        lbl.add_css_class("soundListEmpty");
        let row = gtk4::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        row.set_child(Some(&lbl));
        list_rc.append(&row);
        *placeholder.borrow_mut() = Some(row);
    }
}

fn clear_placeholder(list_rc: &Rc<gtk4::ListBox>, placeholder: &Rc<RefCell<Option<gtk4::ListBoxRow>>>) {
    if let Some(row) = placeholder.borrow_mut().take() {
        list_rc.remove(&row);
    }
}

fn ensure_app_placeholder(
    list_rc:     &Rc<gtk4::FlowBox>,
    placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>,
    text:        &str,
) {
    if placeholder.borrow().is_none() {
        let lbl = gtk4::Label::new(Some(text));
        lbl.add_css_class("soundListEmpty");
        let child = gtk4::FlowBoxChild::new();
        child.set_child(Some(&lbl));
        child.set_focusable(false);
        list_rc.insert(&child, -1);
        *placeholder.borrow_mut() = Some(child);
    }
}

fn clear_app_placeholder(list_rc: &Rc<gtk4::FlowBox>, placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>) {
    if let Some(child) = placeholder.borrow_mut().take() {
        list_rc.remove(&child);
    }
}

const SLIDER_TOUCH_GUARD: Duration = Duration::from_millis(900);

fn update_output_rows(
    list_rc:      &Rc<gtk4::ListBox>,
    rows:         &Rc<RefCell<HashMap<String, SliderRowHandles>>>,
    placeholder:  &Rc<RefCell<Option<gtk4::ListBoxRow>>>,
    radio_group:  &Rc<RefCell<Option<gtk4::CheckButton>>>,
    devices:      &[OutputDevice],
) {
    let mut map = rows.borrow_mut();

    if devices.is_empty() {
        ensure_placeholder(list_rc, placeholder, "No output devices found");
        for (_, handle) in map.drain() {
            list_rc.remove(&handle.row);
        }
        *radio_group.borrow_mut() = None;
        return;
    }
    clear_placeholder(list_rc, placeholder);

    let seen: std::collections::HashSet<String> = devices.iter().map(|d| d.id.clone()).collect();

    for dev in devices {
        if let Some(handle) = map.get(&dev.id) {
            let recently_touched = handle.last_touch.get().elapsed() < SLIDER_TOUCH_GUARD;
            if !recently_touched {
                let cur = handle.scale.value().round() as u32;
                if cur != dev.volume {
                    handle.scale.block_signal(&handle.handler);
                    handle.scale.set_value(dev.volume as f64);
                    handle.scale.unblock_signal(&handle.handler);
                    handle.value_lbl.set_label(&format!("{}%", dev.volume));
                }
            }
            if dev.is_default {
                handle.name_lbl.add_css_class("soundListDefault");
            } else {
                handle.name_lbl.remove_css_class("soundListDefault");
            }
            if handle.name_lbl.label().to_string() != dev.label {
                handle.name_lbl.set_label(&dev.label);
            }
            if let (Some(radio), Some(radio_handler)) = (&handle.radio, &handle.radio_handler) {
                if radio.is_active() != dev.is_default {
                    radio.block_signal(radio_handler);
                    radio.set_active(dev.is_default);
                    radio.unblock_signal(radio_handler);
                }
            }
        } else {
            let on_change: Rc<dyn Fn(&str, u32)> = Rc::new(|id: &str, v: u32| set_output_volume(id, v));
            let radio_cfg = RadioConfig {
                group_leader: radio_group.clone(),
                on_select: Rc::new(|id: &str| set_default_sink(id)),
            };
            let handle = build_sound_row(
                dev.id.clone(),
                dev.label.clone(),
                dev.volume,
                dev.is_default,
                on_change,
                Some(radio_cfg),
            );
            list_rc.append(&handle.row);
            map.insert(dev.id.clone(), handle);
        }
    }

    let stale: Vec<String> = map.keys().filter(|k| !seen.contains(*k)).cloned().collect();
    for k in stale {
        if let Some(handle) = map.remove(&k) {
            list_rc.remove(&handle.row);
        }
    }
}

fn update_app_rows(
    list_rc:     &Rc<gtk4::FlowBox>,
    rows:        &Rc<RefCell<HashMap<String, AppTileHandles>>>,
    placeholder: &Rc<RefCell<Option<gtk4::FlowBoxChild>>>,
    apps:        &[AppPlayback],
) {
    let mut map = rows.borrow_mut();

    if apps.is_empty() {
        ensure_app_placeholder(list_rc, placeholder, "No apps are playing audio");
        for (_, handle) in map.drain() {
            list_rc.remove(&handle.child);
        }
        return;
    }
    clear_app_placeholder(list_rc, placeholder);

    let seen: std::collections::HashSet<String> = apps.iter().map(|a| a.id.clone()).collect();

    for app in apps {
        if let Some(handle) = map.get(&app.id) {
            let recently_touched = handle.last_touch.get().elapsed() < SLIDER_TOUCH_GUARD;
            if !recently_touched && handle.volume.get() != app.volume {
                handle.volume.set(app.volume);
                handle.value_lbl.set_label(&format!("{}%", app.volume));
            }
            if handle.name_lbl.label().to_string() != app.label {
                handle.name_lbl.set_label(&app.label);
            }
        } else {
            let on_change: Rc<dyn Fn(&str, u32)> = Rc::new(|id: &str, v: u32| set_app_volume(id, v));
            let handle = build_app_row(app.id.clone(), app.label.clone(), app.icon_name.clone(), app.volume, on_change);
            list_rc.insert(&handle.child, -1);
            map.insert(app.id.clone(), handle);
        }
    }

    let stale: Vec<String> = map.keys().filter(|k| !seen.contains(*k)).cloned().collect();
    for k in stale {
        if let Some(handle) = map.remove(&k) {
            list_rc.remove(&handle.child);
        }
    }
}

fn wifi_soft_blocked() -> bool {
    let Ok(entries) = std::fs::read_dir("/sys/class/rfkill") else { return false };
    for entry in entries.flatten() {
        let base = entry.path();
        let type_path = base.join("type");
        let soft_path = base.join("soft");
        if std::fs::read_to_string(&type_path)
            .map(|t| t.trim() == "wlan")
            .unwrap_or(false)
        {
            if std::fs::read_to_string(&soft_path)
                .map(|s| s.trim() == "1")
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

fn has_internet() -> bool {
    let route_ok = std::fs::read_to_string("/proc/net/route")
        .map(|content| {
            content.lines().skip(1).any(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                cols.len() >= 2 && cols[1] == "00000000"
            })
        })
        .unwrap_or(false);

    if !route_ok {
        return false;
    }

    std::net::TcpStream::connect_timeout(
        &"1.1.1.1:53".parse().unwrap(),
        Duration::from_secs(2),
    )
    .is_ok()
}

fn wifi_ssid(iface: &str) -> Option<String> {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID", "dev", "wifi"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(ssid) = line.strip_prefix("yes:") {
                let s = ssid.trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
    }

    if let Ok(out) = std::process::Command::new("iw")
        .args(["dev", iface, "link"])
        .output()
    {
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            if let Some(ssid) = line.strip_prefix("SSID:") {
                let s = ssid.trim().to_string();
                if !s.is_empty() { return Some(s); }
            }
        }
    }

    if let Ok(out) = std::process::Command::new("iwgetid")
        .args([iface, "-r"])
        .output()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() { return Some(s); }
    }

    None
}

fn get_network_state() -> NetworkState {
    let Ok(entries) = std::fs::read_dir("/sys/class/net") else {
        return NetworkState::Disconnected;
    };

    let mut wifi_up:     Option<String> = None;
    let mut eth_up:      Option<String> = None;
    let mut wifi_exists: bool           = false;

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "lo" { continue; }

        let operstate = std::fs::read_to_string(
            format!("/sys/class/net/{}/operstate", name),
        )
        .unwrap_or_default();
        let is_up = operstate.trim() == "up";

        let is_wireless = entry.path().join("wireless").exists();

        if is_wireless {
            wifi_exists = true;
            if is_up { wifi_up = Some(name.clone()); }
        } else if name.starts_with('e') && is_up {
            eth_up = Some(name.clone());
        }
    }

    if wifi_exists && wifi_up.is_none() && wifi_soft_blocked() {
        return NetworkState::WifiOff;
    }

    let connected = wifi_up.is_some() || eth_up.is_some();
    if !connected {
        return NetworkState::Disconnected;
    }

    if !has_internet() {
        return NetworkState::NoInternet;
    }

    if let Some(iface) = eth_up {
        return NetworkState::EthernetConnected(iface);
    }

    let iface = wifi_up.unwrap();
    let ssid = wifi_ssid(&iface).unwrap_or_else(|| iface.clone());
    NetworkState::WifiConnected(ssid)
}

#[derive(Clone)]
pub struct NetworkHub {
    state: Arc<Mutex<NetworkState>>,
}

impl NetworkHub {
    pub fn new(interval: Duration) -> Self {
        let state = Arc::new(Mutex::new(get_network_state()));
        {
            let state = state.clone();
            std::thread::spawn(move || loop {
                let fresh = get_network_state();
                if let Ok(mut guard) = state.lock() {
                    if *guard != fresh {
                        *guard = fresh;
                    }
                }
                std::thread::sleep(interval);
            });
        }
        NetworkHub { state }
    }

    pub fn get(&self) -> NetworkState {
        self.state
            .lock()
            .map(|g| g.clone())
            .unwrap_or(NetworkState::Disconnected)
    }
}

pub fn spawn_wifi_scan() -> std::sync::mpsc::Receiver<Vec<(String, String, bool)>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let nets = get_wifi_networks();
        let _ = tx.send(nets);
    });
    rx
}

fn network_icon_and_tip(state: NetworkState) -> (&'static str, String, String) {
    match state {
        NetworkState::WifiConnected(ssid) => (
            "/var/lib/cynager/icons/wifi.svg",
            "Wifi".to_string(),
            ssid,
        ),
        NetworkState::EthernetConnected(iface) => (
            "/var/lib/cynager/icons/ethernet.svg",
            "Ethernet".to_string(), 
            iface,
        ),
        NetworkState::NoInternet => (
            "/var/lib/cynager/icons/nointernet.svg",
            "Connected".to_string(),
            "with No Internet".to_string(),
        ),
        NetworkState::Disconnected => (
            "/var/lib/cynager/icons/disconnected.svg",
            "Disconnected".to_string(),
            "Connect to a network?".to_string(),
        ),
        NetworkState::WifiOff => (
            "/var/lib/cynager/icons/wifioff.svg",
            "Network OFF".to_string(),
            "Turn on Network?".to_string(),
        ),
    }
}

fn toggle_wifi_adapter(enable: bool) {
    let action = if enable { "unblock" } else { "block" };
    let _ = std::process::Command::new("rfkill")
        .args([action, "wifi"])
        .spawn();
}

fn get_wifi_networks() -> Vec<(String, String, bool)> {
    if let Ok(out) = std::process::Command::new("nmcli")
        .args(["-t", "-f", "ACTIVE,SSID,SIGNAL,SECURITY", "dev", "wifi", "list"])
        .output()
    {
        let mut nets: Vec<(String, String, bool)> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(4, ':').collect();
                if parts.len() >= 3 {
                    let active = parts[0] == "yes";
                    let ssid = parts[1].trim().to_string();
                    if ssid.is_empty() { return None; }
                    let signal: u32 = parts[2].trim().parse().unwrap_or(0);
                    let bars = match signal {
                        0..=20  => "▌",
                        21..=40 => "▌ ▌",
                        41..=60 => "▌ ▌ ▌",
                        _       => "▌ ▌ ▌ ▌",
                    };
                    Some((ssid, bars.to_string(), active))
                } else { None }
            })
            .collect();
        nets.sort_by(|a, b| b.2.cmp(&a.2));
        return nets;
    }
    return vec![]
}

fn render_network_rows(
    net_list_rc:  &Rc<gtk4::ListBox>,
    networks:     Vec<(String, String, bool)>,
    netbtn:       &Button,
    net_panel:    &Rc<GtkBox>,
    net_expanded: &Rc<RefCell<bool>>,
) {
    while let Some(child) = net_list_rc.first_child() {
        net_list_rc.remove(&child);
    }

    if networks.is_empty() {
        let row = gtk4::Label::new(Some("No networks found"));
        row.add_css_class("netListEmpty");
        net_list_rc.append(&row);
        return;
    }

    for (ssid, bars, active) in networks {
        let row_box = GtkBox::new(Orientation::Horizontal, 10);
        row_box.add_css_class("netListRow");

        let ssid_lbl = gtk4::Label::new(Some(&ssid));
        ssid_lbl.set_hexpand(true);
        ssid_lbl.set_halign(gtk4::Align::Start);
        ssid_lbl.add_css_class("netListSSID");

        let signal_lbl = gtk4::Label::new(Some(&bars));
        signal_lbl.add_css_class("netListSignal");

        if active {
            let connected_lbl = gtk4::Label::new(Some("•"));
            connected_lbl.add_css_class("netListConnected");
            row_box.append(&connected_lbl);
        }
        row_box.append(&ssid_lbl);
        row_box.append(&signal_lbl);

        let row_btn = Button::builder()
            .child(&row_box)
            .css_classes(["netListRowBtn"])
            .build();

        let ssid_clone         = ssid.clone();
        let netbtn_click       = netbtn.clone();
        let net_panel_click    = net_panel.clone();
        let net_expanded_click = net_expanded.clone();

        row_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("nmcli")
                .args(["dev", "wifi", "connect", &ssid_clone])
                .spawn();
            netbtn_click.remove_css_class("netBtnExpanded");
            net_panel_click.add_css_class("closeBox");
            *net_expanded_click.borrow_mut() = false;
            let net_panel_close = net_panel_click.clone();
            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                net_panel_close.remove_css_class("closeBox");
                net_panel_close.set_visible(false);
            });
        });
        net_list_rc.append(&row_btn);
    }
}

fn build_round_user_icon(pixbuf: Pixbuf, size: i32) -> DrawingArea {
    let icon = DrawingArea::new();
    icon.set_content_width(size);
    icon.set_content_height(size);

    icon.set_draw_func(move |_, cr, w, h| {
        let w = w as f64;
        let h = h as f64;
        let cx = w / 2.0;
        let cy = h / 2.0;
        let r  = w / 2.0;

        cr.arc(cx, cy, r, 0.0, 2.0 * std::f64::consts::PI);
        cr.clip();

        let pb = pixbuf.scale_simple(w as i32, h as i32, gtk4::gdk_pixbuf::InterpType::Bilinear).unwrap();
        cr.set_source_pixbuf(&pb, 0.0, 0.0);
        cr.paint().unwrap();

        let shine = gtk4::cairo::LinearGradient::new(
            cx * 0.35, cy * 0.10,
            cx * 0.80, cy * 0.75,
        );
        shine.add_color_stop_rgba(0.00, 1.0, 1.0, 1.0, 0.55);
        shine.add_color_stop_rgba(0.40, 1.0, 1.0, 1.0, 0.18);
        shine.add_color_stop_rgba(1.00, 1.0, 1.0, 1.0, 0.00);

        cr.set_source(&shine).unwrap();

        cr.save().unwrap();
        cr.translate(cx, cy);
        cr.scale(r * 0.85, r * 0.55);
        cr.translate(-r * 0.08, -r * 0.80);
        cr.arc(0.0, 0.0, 1.0, 0.0, 2.0 * std::f64::consts::PI);
        cr.restore().unwrap();
        cr.fill().unwrap();

        let rim = gtk4::cairo::LinearGradient::new(cx * 0.4, 0.0, cx * 1.6, r * 0.18);
        rim.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.00);
        rim.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.45);
        rim.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.00);
        cr.set_source(&rim).unwrap();
        cr.arc(cx, cy, r - 0.5, std::f64::consts::PI * 1.15, std::f64::consts::PI * 1.85);
        cr.set_line_width(1.5);
        cr.stroke().unwrap();
    });

    icon
}

fn parse_probe_ver_block(path: &str) -> Vec<(String, String)> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("[ctrl] Error reading probe file {}: {}", path, err);
            return Vec::new();
        }
    };

    let mut in_block = false;
    let mut entries  = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == ":ver" {
            in_block = true;
            continue;
        }

        if trimmed == ":end" {
            if in_block {
                break;
            }
            continue;
        }

        if in_block && !trimmed.is_empty() {
            if let Some((name, ver)) = trimmed.split_once(':') {
                let name = name.trim().to_string();
                let ver  = ver.trim().to_string();
                if !name.is_empty() {
                    entries.push((name, ver));
                }
            }
        }
    }

    entries
}

fn build_ver_row(name: &str, version: &str) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::new();
    row.set_selectable(false);
    row.set_activatable(false);
    row.add_css_class("verListRow");

    let row_box = GtkBox::new(Orientation::Horizontal, 10);
    row_box.add_css_class("verListRowBox");

    let name_lbl = Label::builder()
        .label(name)
        .css_classes(if name == "cynageOS" {
            ["cynverListName"]
        } else {
            ["verListName"]
        } )
        .halign(gtk4::Align::Start)
        .hexpand(true)
        .build();

    let ver_lbl = Label::builder()
        .label(version)
        .css_classes(["verListValue"])
        .halign(gtk4::Align::End)
        .build();

    row_box.append(&name_lbl);
    row_box.append(&ver_lbl);
    row.set_child(Some(&row_box));
    row
}

fn populate_ver_table(list_rc: &Rc<gtk4::ListBox>) {
    while let Some(child) = list_rc.first_child() {
        list_rc.remove(&child);
    }

    let entries = parse_probe_ver_block("/var/lib/cynager/info.probe");

    if entries.is_empty() {
        let row = gtk4::ListBoxRow::new();
        row.set_selectable(false);
        row.set_activatable(false);
        let lbl = Label::new(Some("No version info found"));
        lbl.add_css_class("netListEmpty");
        row.set_child(Some(&lbl));
        list_rc.append(&row);
        return;
    }

    for (name, version) in entries {
        list_rc.append(&build_ver_row(&name, &version));
    }
}

fn is_airplane() -> bool {
    let output = std::process::Command::new("rfkill")
        .arg("list")
        .output()
        .expect("[ctrl] failed to run rfkill");

    let mut wifi_on = true;
    let mut blue_on = true;
    let mut tyype = "";

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let line = line.trim();
        
        if line.to_lowercase().contains("bluetooth") {
            tyype = "bluet";
        } else if line.to_lowercase().contains("wireless lan") {
            tyype = "wifi";
        } else if line.starts_with("Soft blocked:") {
            match tyype {
                "wifi" => {
                    let ye_no = line.strip_prefix("Soft blocked:").expect("[ctrl] stripping error :/");
                    wifi_on &= ye_no.trim() == "no";
                }
                "bluet" => {
                    let ye_no = line.strip_prefix("Soft blocked:").expect("[ctrl] the other stripping error :|");
                    blue_on &= ye_no.trim() == "no";
                }
                _ => {}
            }
        }
    }


    if !wifi_on && !blue_on {
        return true
    }

    false
}

fn is_dnd() -> bool {
    let file = File::open("/var/lib/cynager/info.probe").expect("[ctrl] info probe file not found");
    let mut sett_block = false;
    for line in BufReader::new(file).lines() {
        let line = line.expect("[ctrl] reading tru lines error");
        let line = line.trim();

        if sett_block && line.starts_with("dnd") {
            let ye_no = line.strip_prefix("dnd :").expect("[ctrl] cant strip in settings block");
            if ye_no.trim() == "true" {
                return true
            }
        }

        if line.starts_with(":set") {
            sett_block = true;
        }
    }

    false
}

fn toggle_dnd() {
    let path = "/var/lib/cynager/info.probe";
    let tmp_path = "/var/lib/cynager/info.probe.tmp";

    let file = File::open(path).expect("[ctrl] info probe file not found");
    let reader = BufReader::new(file);

    let out = File::create(tmp_path).expect("[ctrl] cant create tmp file");
    let mut writer = BufWriter::new(out);

    let mut sett_block = false;

    for line in reader.lines() {
        let line = line.expect("[ctrl] reading line error");
        let trimmed = line.trim();

        if trimmed == ":set" {
            sett_block = true;
            writeln!(writer, "{}", line).expect("[ctrl] write error");
            continue;
        }

        if trimmed == ":end" {
            sett_block = false;
            writeln!(writer, "{}", line).expect("[ctrl] write error");
            continue;
        }

        if sett_block && trimmed.starts_with("dnd") {
            let ye_no = trimmed.strip_prefix("dnd :")
                .expect("[ctrl] cant strip in settings block")
                .trim();

            let flipped = if ye_no == "true" { "false" } else { "true" };

            let indent_len = line.len() - line.trim_start().len();
            let indent = &line[..indent_len];

            writeln!(writer, "{}dnd :{}", indent, flipped).expect("[ctrl] write error");
            continue;
        }

        writeln!(writer, "{}", line).expect("[ctrl] write error");
    }

    writer.flush().expect("[ctrl] flush error");
    std::fs::rename(tmp_path, path).expect("[ctrl] cant replace original file");
}

pub fn spawn_ctrl_capsules(
    app:          &Application,
    overlay_open: Rc<RefCell<bool>>,
    net_hub:      NetworkHub,
) -> ApplicationWindow {
    let win = ApplicationWindow::builder()
        .application(app)
        .title("capsuleCTRL")
        .css_classes(["ctrlOverlay"])
        .build();
 
    // *overlay_open.borrow_mut() = true; 

    win.init_layer_shell();
    win.set_namespace(Some("CtrlOverlay"));
    win.set_layer(Layer::Overlay);
    win.remove_css_class("background");
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Bottom,true);
    win.set_anchor(Edge::Left,true);
    win.set_anchor(Edge::Right,true);
    // win.set_exclusive_zone(200);
    // win.auto_exclusive_zone_enable();
    
    let top_backdrop = Button::builder()
        .css_classes(["ctrlBackdrop"])
        .hexpand(true)
        .vexpand(false)
        .height_request(50)
        .build();

    let backdrop = Button::builder()
        .css_classes(["ctrlBackdrop"])
        .hexpand(true)
        .vexpand(true)
        .build();
    
    let dir_path = "/usr/share/octobacillus/";
    let base_name = "usericon.";
    let valid_extensions = ["png", "jpeg", "jpg"];

    let mut final_path = String::from("/usr/share/octobacillus/usericon.png"); 

    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                if file_name.starts_with(base_name) {
                    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                        if valid_extensions.contains(&ext.to_lowercase().as_str()) {
                            final_path = path.to_string_lossy().into_owned();
                            break;
                        }
                    }
                }
            }
        }
    }


    let pixbuf = Pixbuf::from_file(&final_path).unwrap();
    let usricon = build_round_user_icon(pixbuf.clone(), 35);

    
    let usrname = match std::fs::read_to_string("/usr/share/octobacillus/user.octo") {
        Ok(content) => content,
        Err(err) => {
            eprintln!("[ctrl] Error reading file: {}", err);
            "name = user4.0".to_string()
        }
    };

    let name = usrname
        .lines()
        .find(|line| line.trim().starts_with("name"))
        .and_then(|line| line.split_once("="))
        .map(|(_, value)| value.trim().to_string())
        .unwrap_or_else(|| "user4.0".to_string());

    let usrbox = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    usrbox.append(&usricon);

    let usr_labels = GtkBox::new(Orientation::Vertical, 2);
    usr_labels.append(&Label::builder()
        .label("Hello,")
        .css_classes(["userHello"])
        .halign(gtk4::Align::Start)
        .build()
    );
    usr_labels.append(&Label::builder()
        .label(name)
        .css_classes(["userName"])
        .halign(gtk4::Align::Start)
        .build()
    );

    usrbox.append(&usr_labels);
    
    let usr = Button::builder()
        .child(&usrbox)
        .css_classes(["ctrlBtnL"])
        .build();

    let usr_panel_icon = build_round_user_icon(pixbuf.clone(), 72);
    usr_panel_icon.add_css_class("userPanelIcon");

    let usr_panel_power_icon = Image::from_file("/var/lib/cynager/icons/cos-shutdown.svg");
    usr_panel_power_icon.set_icon_size(gtk4::IconSize::Large);

    let usr_panel_power_btn = Button::builder()
        .child(&usr_panel_power_icon)
        .css_classes(["dockBtn"])
        .tooltip_text("Power")
        .valign(gtk4::Align::Center)
        .build();

    {
        usr_panel_power_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("terminatee").spawn();
        });
    }

    let usr_panel_dummy_fill = GtkBox::new(Orientation::Horizontal, 0);
    usr_panel_dummy_fill.set_hexpand(true);

    let usr_panel_actions = GtkBox::new(Orientation::Horizontal, 8);
    usr_panel_actions.add_css_class("userPanelActions");
    usr_panel_actions.append(&usr_panel_icon);
    usr_panel_actions.append(&usr_panel_dummy_fill);
    usr_panel_actions.append(&usr_panel_power_btn);

    let ver_list_box = gtk4::ListBox::new();
    ver_list_box.add_css_class("ctrlpanelList");
    ver_list_box.add_css_class("verList");
    ver_list_box.set_selection_mode(gtk4::SelectionMode::None);
    let ver_list_rc = Rc::new(ver_list_box);

    let ver_scroll = gtk4::ScrolledWindow::new();
    ver_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    ver_scroll.set_max_content_height(220);
    ver_scroll.set_propagate_natural_height(true);
    ver_scroll.set_child(Some(&*ver_list_rc));
    ver_scroll.add_css_class("netListScroll");

    let user_panel = GtkBox::new(Orientation::Vertical, 6);
    user_panel.add_css_class("ctrlPanel");
    user_panel.append(&usr_panel_actions);
    user_panel.append(&ver_scroll);
    user_panel.set_visible(false);

    let user_panel_rc = Rc::new(user_panel);
    let user_expanded = Rc::new(RefCell::new(false));

    let initial_state = net_hub.get();
    let (init_icon, init_label, init_body) = network_icon_and_tip(initial_state.clone());

    let net_icon  = Image::from_file(init_icon);
    net_icon.set_icon_size(gtk4::IconSize::Large);
    let net_label = Label::new(Some(&init_label));
    net_label.add_css_class("netBtnLabel");
    net_label.set_halign(gtk4::Align::Start);
    let net_body = Label::new(Some(&init_body));
    net_body.add_css_class("netBtnBody");
    net_body.set_halign(gtk4::Align::Start);

    let net_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    
    let net_labels_box = GtkBox::new(Orientation::Vertical, 2);
    net_labels_box.append(&net_label);
    net_labels_box.append(&net_body);

    net_box.append(&net_icon);
    net_box.append(&net_labels_box);

    let netbtn = Button::builder()
        .child(&net_box)
        .css_classes(["ctrlBtnL"])
        .build();

    let net_icon_rc  = Rc::new(net_icon);
    let net_label_rc = Rc::new(net_label);
    let net_body_rc  = Rc::new(net_body);

    let net_expanded = Rc::new(RefCell::new(false));

    let wifi_toggle_btn = Switch::builder()
        .active(!wifi_soft_blocked())
        .css_classes(["netPanelSwitch"])
        .tooltip_text("Toggle WiFi adapter")
        .valign(gtk4::Align::Center)
        .margin_start(10)
        .build();

    let dummy_fill = GtkBox::new(Orientation::Horizontal, 0);
    dummy_fill.set_hexpand(true);

    let refresh_icon = Image::from_file("/var/lib/cynager/icons/frsh.svg");
    refresh_icon.set_icon_size(gtk4::IconSize::Normal);
    let refresh_btn = Button::builder()
        .child(&refresh_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Refresh networks")
        .build();

    let net_settings_icon = Image::from_file("/var/lib/cynager/icons/cog.svg");
    net_settings_icon.set_icon_size(gtk4::IconSize::Normal);
    let net_settings_btn = Button::builder()
        .child(&net_settings_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Network settings")
        .build();

    let net_panel_actions = GtkBox::new(Orientation::Horizontal, 8);
    net_panel_actions.add_css_class("netPanelActions");
    net_panel_actions.append(&wifi_toggle_btn);
    net_panel_actions.append(&dummy_fill);
    net_panel_actions.append(&refresh_btn);
    net_panel_actions.append(&net_settings_btn);

    let net_list_box = gtk4::ListBox::new();
    net_list_box.add_css_class("ctrlpanelList");
    net_list_box.set_selection_mode(gtk4::SelectionMode::None);


    let net_list_rc = Rc::new(net_list_box);
    let scroll_win = gtk4::ScrolledWindow::new();
    scroll_win.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    scroll_win.set_max_content_height(220);
    scroll_win.set_propagate_natural_height(true);
    scroll_win.set_child(Some(&*net_list_rc));
    scroll_win.add_css_class("netListScroll");

    let net_panel = GtkBox::new(Orientation::Vertical, 6);
    net_panel.add_css_class("ctrlPanel");
    net_panel.append(&net_panel_actions);
    net_panel.append(&scroll_win);
    net_panel.set_visible(false);

    let net_panel_rc = Rc::new(net_panel);

    let scan_and_render = {
        let net_list_rc = net_list_rc.clone();
        let netbtn_inner = netbtn.clone();
        let net_panel_inner = net_panel_rc.clone();
        let net_expanded_inner = net_expanded.clone();
        move |on_done: Option<Rc<dyn Fn()>>| {
            while let Some(child) = net_list_rc.first_child() {
                net_list_rc.remove(&child);
            }
            let row = gtk4::Label::new(Some("Searching..."));
            row.add_css_class("netListEmpty");
            net_list_rc.append(&row);

            let rx = spawn_wifi_scan();
            let net_list_rc = net_list_rc.clone();
            let netbtn_inner = netbtn_inner.clone();
            let net_panel_inner = net_panel_inner.clone();
            let net_expanded_inner = net_expanded_inner.clone();

            glib::timeout_add_local(Duration::from_millis(80), move || {
                match rx.try_recv() {
                    Ok(networks) => {
                        render_network_rows(
                            &net_list_rc,
                            networks,
                            &netbtn_inner,
                            &net_panel_inner,
                            &net_expanded_inner,
                        );
                        if let Some(cb) = &on_done { cb(); }
                        glib::ControlFlow::Break
                    }
                    Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                    Err(TryRecvError::Disconnected) => {
                        if let Some(cb) = &on_done { cb(); }
                        glib::ControlFlow::Break
                    }
                }
            });
        }
    };
    let scan_and_render_rc = Rc::new(scan_and_render);
    let populate_networks_rc = {
        let scan_and_render_rc = scan_and_render_rc.clone();
        Rc::new(move || scan_and_render_rc(None))
    };

    {
        let scan_and_render_rc = scan_and_render_rc.clone();
        let net_list_rc = net_list_rc.clone();
        let refresh_btn_clone = refresh_btn.clone();
        wifi_toggle_btn.connect_state_set(move |_, state| {
            toggle_wifi_adapter(state);

            if !state {
                while let Some(child) = net_list_rc.first_child() {
                    net_list_rc.remove(&child);
                }
                let row = gtk4::Label::new(Some("Wi-Fi is off"));
                row.add_css_class("netListEmpty");
                net_list_rc.append(&row);
            } else {
                refresh_btn_clone.add_css_class("spinning");
                let refresh_btn_clone_inner = refresh_btn_clone.clone();
                let scan_and_render_rc = scan_and_render_rc.clone();
                glib::timeout_add_local_once(Duration::from_millis(1500), move || {
                    let _ = std::process::Command::new("nmcli")
                        .args(["dev", "wifi", "rescan"])
                        .spawn();
                    let refresh_btn_done = refresh_btn_clone_inner.clone();
                    scan_and_render_rc(Some(Rc::new(move || {
                        refresh_btn_done.remove_css_class("spinning");
                    })));
                });
            }

            glib::Propagation::Proceed
        });
    }

    {
        let scan_and_render_rc = scan_and_render_rc.clone();
        refresh_btn.connect_clicked(move |btn| {
            btn.add_css_class("spinning");
            let _ = std::process::Command::new("nmcli")
                .args(["dev", "wifi", "rescan"])
                .spawn();
            let btn_done = btn.clone();
            let scan_and_render_rc = scan_and_render_rc.clone();
            glib::timeout_add_local_once(Duration::from_millis(600), move || {
                scan_and_render_rc(Some(Rc::new(move || {
                    btn_done.remove_css_class("spinning");
                })));
            });
        });
    }

    {
        net_settings_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("nm-connection-editor").spawn();
        });
    }

    let net_last_shown = Rc::new(RefCell::new(initial_state));

    {
        let net_icon_rc  = net_icon_rc.clone();
        let net_label_rc = net_label_rc.clone();
        let net_body_rc  = net_body_rc.clone();
        let net_hub      = net_hub.clone();
        let net_last_shown = net_last_shown.clone();

        glib::timeout_add_local(Duration::from_millis(500), move || {
            let state = net_hub.get();
            let mut last = net_last_shown.borrow_mut();
            if *last != state {
                let (icon_name, label_text, label_body) = network_icon_and_tip(state.clone());
                net_icon_rc.set_from_file(Some(icon_name));
                net_label_rc.set_label(&label_text);
                net_body_rc.set_label(&label_body);
                *last = state;
            }
            glib::ControlFlow::Continue
        });
    }

    let sound_rx = spawn_sound_watcher(Duration::from_secs(3));
    let init_snd = SoundState {
        volume: 0,
        muted: false,
        sink: "Loading audio…".to_string(),
        mic_volume: 0,
        mic_muted: false,
    };

    let snd_icon = Image::from_file(sound_icon(&init_snd));
    snd_icon.set_icon_size(gtk4::IconSize::Large);

    let snd_label = Label::new(Some(&format!("{}%", init_snd.volume)));
    snd_label.add_css_class("netBtnLabel");
    snd_label.set_halign(gtk4::Align::Start);

    let snd_body = Label::new(Some(&init_snd.sink));
    snd_body.add_css_class("netBtnBody");
    snd_body.set_halign(gtk4::Align::Start);

    let snd_labels_box = GtkBox::new(Orientation::Vertical, 2);
    snd_labels_box.append(&snd_label);
    snd_labels_box.append(&snd_body);

    let sound_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();
    sound_box.append(&snd_icon);
    sound_box.append(&snd_labels_box);

    let soundbtn = Button::builder()
        .child(&sound_box)
        .css_classes(["ctrlBtnL"])
        .build();

    let snd_icon_rc = Rc::new(snd_icon);
    let snd_label_rc = Rc::new(snd_label);
    let snd_body_rc = Rc::new(snd_body);
    let sound_rx = Rc::new(RefCell::new(sound_rx));
    let sound_cache: Rc<RefCell<Option<SoundState>>> = Rc::new(RefCell::new(None));

    let volume_icon = Image::from_file(sound_icon(&init_snd));
    volume_icon.set_icon_size(gtk4::IconSize::Normal);
    volume_icon.add_css_class("soundPanelIcon");
    volume_icon.set_valign(gtk4::Align::Center);
    volume_icon.set_margin_start(10);

    let mic_icon = Image::from_file(if init_snd.mic_muted {
        "/var/lib/cynager/icons/micmute.svg"
    } else {
        "/var/lib/cynager/icons/micon.svg"
    });
    mic_icon.set_icon_size(gtk4::IconSize::Normal);
    mic_icon.add_css_class("soundPanelIcon");
    mic_icon.set_valign(gtk4::Align::Center);

    let volume_icon_rc = Rc::new(volume_icon);
    let mic_icon_rc = Rc::new(mic_icon);

    let mute_toggle = Switch::builder()
        .active(!init_snd.muted)
        .css_classes(["soundPanelSwitch"])
        .tooltip_text("Toggle Sound")
        .valign(gtk4::Align::Center)
        .build();

    let mic_toggle = Switch::builder()
        .active(!init_snd.mic_muted)
        .css_classes(["soundPanelSwitch"])
        .tooltip_text("Toggle Microphone")
        .valign(gtk4::Align::Center)
        .build();

    let mute_toggle_rc = Rc::new(mute_toggle);
    let mic_toggle_rc  = Rc::new(mic_toggle);

    let mute_toggle_handler = Rc::new(mute_toggle_rc.connect_state_set(move |_, state| {
        set_master_mute(!state);
        glib::Propagation::Proceed
    }));
    let mic_toggle_handler = Rc::new(mic_toggle_rc.connect_state_set(move |_, state| {
        set_mic_mute(!state);
        glib::Propagation::Proceed
    }));

    let sound_settings_icon = Image::from_file("/var/lib/cynager/icons/cog.svg");
    sound_settings_icon.set_icon_size(gtk4::IconSize::Normal);
    let sound_settings_btn = Button::builder()
        .child(&sound_settings_icon)
        .css_classes(["netPanelBtn"])
        .tooltip_text("Sound settings")
        .build();

    {
        sound_settings_btn.connect_clicked(move |_| {
            let _ = std::process::Command::new("pavucontrol").spawn();
        });
    }

    let sound_dummy_fill = GtkBox::new(Orientation::Horizontal, 0);
    sound_dummy_fill.set_hexpand(true);

    let sound_panel_actions = GtkBox::new(Orientation::Horizontal, 8);
    sound_panel_actions.add_css_class("soundPanelActions");
    sound_panel_actions.append(&*volume_icon_rc);
    sound_panel_actions.append(&*mute_toggle_rc);
    sound_panel_actions.append(&*mic_icon_rc);
    sound_panel_actions.append(&*mic_toggle_rc);
    sound_panel_actions.append(&sound_dummy_fill);
    sound_panel_actions.append(&sound_settings_btn);

    let output_list_box = gtk4::ListBox::new();
    output_list_box.add_css_class("ctrlpanelList");
    output_list_box.set_selection_mode(gtk4::SelectionMode::None);
    let output_list_rc = Rc::new(output_list_box);
    let output_scroll = gtk4::ScrolledWindow::new();
    output_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    output_scroll.set_max_content_height(220);
    output_scroll.set_propagate_natural_height(true);
    output_scroll.set_child(Some(&*output_list_rc));
    output_scroll.add_css_class("netListScroll");

    let apps_list_box = gtk4::FlowBox::new();
    apps_list_box.add_css_class("ctrlpanelList");
    apps_list_box.add_css_class("appsGrid");
    apps_list_box.set_selection_mode(gtk4::SelectionMode::None);
    apps_list_box.set_homogeneous(true);
    apps_list_box.set_min_children_per_line(2);
    apps_list_box.set_max_children_per_line(2);
    apps_list_box.set_row_spacing(5);
    apps_list_box.set_column_spacing(8);
    let apps_list_rc = Rc::new(apps_list_box);
    let apps_scroll = gtk4::ScrolledWindow::new();
    apps_scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
    apps_scroll.set_max_content_height(220);
    apps_scroll.set_propagate_natural_height(true);
    apps_scroll.set_child(Some(&*apps_list_rc));
    apps_scroll.add_css_class("netListScroll");

    let sound_stack = gtk4::Stack::new();
    sound_stack.set_transition_type(gtk4::StackTransitionType::SlideLeftRight);
    sound_stack.set_transition_duration(150);
    sound_stack.add_titled(&apps_scroll, Some("apps"), "Applications");
    sound_stack.add_titled(&output_scroll, Some("output"), "Output Devices");

    let sound_tabs = gtk4::StackSwitcher::new();
    sound_tabs.set_stack(Some(&sound_stack));
    sound_tabs.add_css_class("soundTabs");

    let output_rows: Rc<RefCell<HashMap<String, SliderRowHandles>>> = Rc::new(RefCell::new(HashMap::new()));
    let output_radio_group: Rc<RefCell<Option<gtk4::CheckButton>>> = Rc::new(RefCell::new(None));
    let output_placeholder: Rc<RefCell<Option<gtk4::ListBoxRow>>> = Rc::new(RefCell::new(None));
    let app_rows: Rc<RefCell<HashMap<String, AppTileHandles>>> = Rc::new(RefCell::new(HashMap::new()));
    let app_placeholder: Rc<RefCell<Option<gtk4::FlowBoxChild>>> = Rc::new(RefCell::new(None));

    let sound_panel = GtkBox::new(Orientation::Vertical, 6);
    sound_panel.add_css_class("ctrlPanel");
    sound_panel.append(&sound_panel_actions);
    sound_panel.append(&sound_tabs);
    sound_panel.append(&sound_stack);
    sound_panel.set_visible(false);

    let sound_panel_rc = Rc::new(sound_panel);
    let sound_expanded  = Rc::new(RefCell::new(false));

    let device_rx = Rc::new(RefCell::new(spawn_sound_devices_watcher(Duration::from_millis(1200))));
    let device_cache: Rc<RefCell<Option<(Vec<OutputDevice>, Vec<AppPlayback>)>>> = Rc::new(RefCell::new(None));

    {
        let device_rx = device_rx.clone();
        let device_cache = device_cache.clone();
        let overlay_open = overlay_open.clone();
        let sound_expanded = sound_expanded.clone();
        let output_list_rc = output_list_rc.clone();
        let apps_list_rc = apps_list_rc.clone();
        let output_rows = output_rows.clone();
        let output_radio_group = output_radio_group.clone();
        let output_placeholder = output_placeholder.clone();
        let app_rows = app_rows.clone();
        let app_placeholder = app_placeholder.clone();

        glib::timeout_add_local(Duration::from_millis(120), move || {
            let latest = {
                let rx = device_rx.borrow();
                let mut latest = None;
                while let Ok(state) = rx.try_recv() {
                    latest = Some(state);
                }
                latest
            };

            if let Some(state) = latest {
                *device_cache.borrow_mut() = Some(state);
            }

            if *overlay_open.borrow() && *sound_expanded.borrow() {
                if let Some((devices, apps)) = device_cache.borrow().as_ref() {
                    update_output_rows(
                        &output_list_rc,
                        &output_rows,
                        &output_placeholder,
                        &output_radio_group,
                        devices,
                    );
                    update_app_rows(&apps_list_rc, &app_rows, &app_placeholder, apps);
                }
            }

            glib::ControlFlow::Continue
        });
    }

    {
        let snd_icon_rc = snd_icon_rc.clone();
        let snd_label_rc = snd_label_rc.clone();
        let snd_body_rc = snd_body_rc.clone();
        let sound_rx = sound_rx.clone();
        let sound_cache_poll = sound_cache.clone();
        let overlay_open = overlay_open.clone();
        let mute_toggle_rc = mute_toggle_rc.clone();
        let mic_toggle_rc = mic_toggle_rc.clone();
        let mute_toggle_handler = mute_toggle_handler.clone();
        let mic_toggle_handler = mic_toggle_handler.clone();
        let volume_icon_rc = volume_icon_rc.clone();
        let mic_icon_rc = mic_icon_rc.clone();

        glib::timeout_add_local(Duration::from_millis(120), move || {
            let rx = sound_rx.borrow();
            let mut latest: Option<SoundState> = None;
            while let Ok(state) = rx.try_recv() {
                latest = Some(state);
            }
            drop(rx);
            if let Some(state) = latest {
                *sound_cache_poll.borrow_mut() = Some(state);
            }

            if *overlay_open.borrow() {
                if let Some(state) = sound_cache_poll.borrow().as_ref() {
                    snd_icon_rc.set_from_file(Some(sound_icon(state)));
                    snd_label_rc.set_label(&format!("{}%", state.volume));
                    snd_body_rc.set_label(&state.sink);
                    volume_icon_rc.set_from_file(Some(sound_icon(&state)));
                    mic_icon_rc.set_from_file(Some(if state.mic_muted {
                        "/var/lib/cynager/icons/micmute.svg"
                    } else {
                        "/var/lib/cynager/icons/micon.svg"
                    }));

                    let want_active = !state.muted;
                    if mute_toggle_rc.is_active() != want_active {
                        mute_toggle_rc.block_signal(&mute_toggle_handler);
                        mute_toggle_rc.set_active(want_active);
                        mute_toggle_rc.set_state(want_active);
                        mute_toggle_rc.unblock_signal(&mute_toggle_handler);
                    }

                    let mic_want_active = !state.mic_muted;
                    if mic_toggle_rc.is_active() != mic_want_active {
                        mic_toggle_rc.block_signal(&mic_toggle_handler);
                        mic_toggle_rc.set_active(mic_want_active);
                        mic_toggle_rc.set_state(mic_want_active);
                        mic_toggle_rc.unblock_signal(&mic_toggle_handler);
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    let airplaneicon = Image::from_file("/var/lib/cynager/icons/wifioff.svg");
    airplaneicon.set_icon_size(gtk4::IconSize::Large);

    let airplane: Button = Button::builder()
        .child(&airplaneicon)
        .css_classes(if is_airplane() {
            ["ctrlExpandedS"]
        } else {
            ["ctrlBtnS"]
        })
        .tooltip_text("Airplane Mode")
        .build();

    let dndicon = Label::builder()
        .label("DnD.")
        .css_classes(["dndicon"])
        .build();

    let dnd: Button = Button::builder()
        .child(&dndicon)
        .css_classes(if is_dnd() {
            ["ctrlExpandedS"]
        } else{ 
            ["ctrlBtnS"]
        })
        .tooltip_text("Toggle Do Not Disturb")
        .build();

    let setticon = Image::from_file("/var/lib/cynager/icons/cog.svg");
    setticon.set_icon_size(gtk4::IconSize::Large);

    let setting: Button = Button::builder()
        .child(&setticon)
        .css_classes(["ctrlBtnS"])
        .tooltip_text("Airplane Mode")
        .build();

    let btns = GtkBox::new(Orientation::Horizontal, 16);
    btns.set_css_classes(&["ctrlBTNSbox"]);
    btns.set_halign(gtk4::Align::Center);
    btns.set_valign(gtk4::Align::Start);
    // btns.set_margin_top(80);
    btns.set_can_target(true);
    btns.append(&usr);
    btns.append(&netbtn);
    btns.append(&airplane);
    btns.append(&dnd);
    btns.append(&setting);
    btns.append(&soundbtn);
    
    btns.add_css_class("startingOSD");

    let ctrl_column = GtkBox::new(Orientation::Vertical, 20);
    ctrl_column.set_halign(gtk4::Align::Center);
    ctrl_column.set_valign(gtk4::Align::Start);
    ctrl_column.append(&top_backdrop);
    ctrl_column.append(&btns);
    ctrl_column.append(&*user_panel_rc);
    ctrl_column.append(&*net_panel_rc);
    ctrl_column.append(&*sound_panel_rc);

    let layout = gtk4::Overlay::new();
    layout.set_child(Some(&backdrop));
    layout.add_overlay(&ctrl_column);
 
    win.set_child(Some(&layout));

    {
        win.connect_visible_notify(move |win| {
            if win.is_visible() {
                btns.add_css_class("startingOSD");
            } else {
                btns.remove_css_class("startingOSD");
            }
        });
    }
 
    let close = {
        let win_c = win.clone();
        let flag = overlay_open.clone();
        Rc::new(move || {
            *flag.borrow_mut() = false;
            win_c.set_visible(false);
        })
    };

    {
        let close_a = close.clone();
        backdrop.connect_clicked(move |_| close_a());
        let close = close.clone();
        top_backdrop.connect_clicked(move |_| close());
    }
 
    {
        let user_panel_rc = user_panel_rc.clone();
        let user_expanded  = user_expanded.clone();
        let usr_c          = usr.clone();
        let ver_list_rc    = ver_list_rc.clone();

        usr.connect_clicked(move |_| {
            let mut expanded = user_expanded.borrow_mut();
            *expanded = !*expanded;
            if *expanded {
                usr_c.set_css_classes(&["ctrlExpanded"]);
                user_panel_rc.set_visible(true);
                populate_ver_table(&ver_list_rc);
            } else {
                usr_c.set_css_classes(&["ctrlBtnL"]);
                user_panel_rc.set_visible(false);
            }
        });
    }
 
    {
        let net_panel_rc  = net_panel_rc.clone();
        let net_expanded  = net_expanded.clone();
        let populate      = populate_networks_rc.clone();
        let netbtn_c      = netbtn.clone();
        netbtn.connect_clicked(move |_| {
            let mut expanded = net_expanded.borrow_mut();
            *expanded = !*expanded;
            if *expanded {
                netbtn_c.set_css_classes(&["ctrlExpanded"]);
                net_panel_rc.set_visible(true);
                populate();
            } else {
                netbtn_c.set_css_classes(&["ctrlBtnL"]);
                net_panel_rc.set_visible(false);
            }
        });
    }

    {
        let airplaneicon_c = airplaneicon.clone();
        airplane.connect_clicked(move |_| {
            if is_airplane() {
                let _ = std::process::Command::new("rfkill")
                    .args(["unblock", "wifi"])
                    .status();

                let _ = std::process::Command::new("rfkill")
                    .args(["unblock", "bluetooth"])
                    .status();
                airplaneicon_c.add_css_class("flyplane");
            } else {
                let _ = std::process::Command::new("rfkill")
                    .args(["block", "wifi"])
                    .status();

                let _ = std::process::Command::new("rfkill")
                    .args(["block", "bluetooth"])
                    .status();
            }
            airplaneicon_c.add_css_class("flyplane");
            let airplaneicon_timeout = airplaneicon_c.clone();
            glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
                airplaneicon_timeout.remove_css_class("flyplane");
                if let Some(airbtn) = airplaneicon_timeout.parent() {
                    airbtn.set_css_classes(if is_airplane() {
                        &["ctrlExpandedS"]
                    }else {
                        &["ctrlBtnS"]
                    });
                }
                glib::ControlFlow::Break 
            });
        });
    }

    {   
        let dnd_clone = dnd.clone();
        dnd.connect_clicked(move |_| {
            toggle_dnd();
            dnd_clone.set_css_classes(if is_dnd() {
                &["ctrlExpandedS"]
            }else {
                &["ctrlBtnS"]
            });
        });
    }

    {
        setting.connect_clicked(move |_| {
            // let _ = std::process::Command::new("nm-connection-editor").spawn();
            // close();
        });
    }

    {
        let sound_panel_rc = sound_panel_rc.clone();
        let sound_expanded = sound_expanded.clone();
        let soundbtn_c = soundbtn.clone();
        let output_list_rc = output_list_rc.clone();
        let apps_list_rc  = apps_list_rc.clone();
        let output_rows = output_rows.clone();
        let output_radio_group = output_radio_group.clone();
        let output_placeholder = output_placeholder.clone();
        let app_rows = app_rows.clone();
        let app_placeholder = app_placeholder.clone();
        let device_cache = device_cache.clone();

        soundbtn.connect_clicked(move |_| {
            let mut expanded = sound_expanded.borrow_mut();
            *expanded = !*expanded;
            if *expanded {
                soundbtn_c.set_css_classes(&["ctrlExpanded"]);
                sound_panel_rc.set_visible(true);

                if let Some((devices, apps)) = device_cache.borrow().as_ref() {
                    update_output_rows(
                        &output_list_rc,
                        &output_rows,
                        &output_placeholder,
                        &output_radio_group,
                        devices,
                    );
                    update_app_rows(&apps_list_rc, &app_rows, &app_placeholder, apps);
                } else {
                    ensure_placeholder(
                        &output_list_rc,
                        &output_placeholder,
                        "Loading output devices…",
                    );
                    ensure_app_placeholder(
                        &apps_list_rc,
                        &app_placeholder,
                        "Loading audio apps…",
                    );
                }
            } else {
                soundbtn_c.set_css_classes(&["ctrlBtnL"]);
                sound_panel_rc.set_visible(false);
            }
        });
    }

    {
        let snd_icon_rc = snd_icon_rc.clone();
        let snd_label_rc = snd_label_rc.clone();
        let sound_cache = sound_cache.clone();

        let scroll = EventControllerScroll::new(
            EventControllerScrollFlags::VERTICAL | EventControllerScrollFlags::DISCRETE,
        );

        scroll.connect_scroll(move |_, _dx, dy| {
            let step = if dy < 0.0 { "5%-" } else { "5%+" };
            let _ = std::process::Command::new("wpctl")
                .args(["set-volume", "-l", "1.0", "@DEFAULT_AUDIO_SINK@", step])
                .spawn();

            if let Some(state) = sound_cache.borrow_mut().as_mut() {
                let delta = if dy < 0.0 { 5 } else { -5 };
                state.volume = (state.volume as i32 + delta).clamp(0, 100) as u32;
                snd_label_rc.set_label(&format!("{}%", state.volume));
                snd_icon_rc.set_from_file(Some(sound_icon(state)));
            }

            glib::Propagation::Stop
        });

        soundbtn.add_controller(scroll);
    }
 
    win.present();
    win.set_visible(false);
    win
}