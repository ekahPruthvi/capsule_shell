use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;
use std::thread;
use crate::widgets::position::{load_positions, save_position};

// i am too lazy to rename this from system to music as i have decided tht battery and other will be seperate and music will be seperate

const NAME: &str = "system";

#[derive(Debug, Clone)]
struct MusicState {
    title:   String,
    artist:  String,
    art_url: String,
    playing: bool,
}



fn pctl(args: &[&str]) -> Option<String> {
    let out = Command::new("playerctl").args(args).output().ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if s.is_empty() { None } else { Some(s) }
}

fn active_player() -> Option<String> {
    let list = pctl(&["--list-all"])?;
    let players: Vec<&str> = list.lines().collect();

    for &p in &players {
        if pctl(&["--player", p, "status"]).as_deref() == Some("Playing") {
            return Some(p.to_owned());
        }
    }
    Some(players[0].to_owned())
}

fn fetch_music_state() -> Option<MusicState> {
    let player = active_player()?;

    let status = pctl(&["--player", &player, "status"]).unwrap_or_default();
    if status == "Stopped" || status.is_empty() {
        return None;
    }

    let title   = pctl(&["--player", &player, "metadata", "title"])?;
    let artist  = pctl(&["--player", &player, "metadata", "artist"]).unwrap_or_default();
    let art_url = pctl(&["--player", &player, "metadata", "mpris:artUrl"]).unwrap_or_default();
    let playing = status == "Playing";

    Some(MusicState { title, artist, art_url, playing })
}

fn fire_playerctl(args: &'static [&'static str]) {
    thread::spawn(move || { let _ = Command::new("playerctl").args(args).status(); });
}

fn spawn_worker<T, W, D>(work: W, on_done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    use std::sync::{Arc, Mutex};
    let slot: Arc<Mutex<Option<T>>> = Arc::new(Mutex::new(None));
    let slot_t = slot.clone();
    thread::spawn(move || {
        let result: T = work();
        if let Ok(mut g) = slot_t.lock() { *g = Some(result); }
    });
    let mut on_done = Some(on_done);
    gtk4::glib::idle_add_local(move || {
        if let Ok(mut g) = slot.try_lock() {
            if let Some(v) = g.take() {
                if let Some(f) = on_done.take() { f(v); }
                return gtk4::glib::ControlFlow::Break;
            }
        }
        gtk4::glib::ControlFlow::Continue
    });
}

pub fn spawn_sys_widget() -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder().title("capsuleWs").build();
    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    win.remove_css_class("background");

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_css_classes(&["starting", "outerSys"]);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandle");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_margin_top(3);
    handle.set_margin_start(80);
    handle.set_margin_end(80);
    handle.set_hexpand(true);

    let music_page = GtkBox::new(Orientation::Vertical, 6);
    music_page.set_margin_start(16);
    music_page.set_margin_end(16);
    music_page.set_margin_top(10);
    music_page.set_margin_bottom(10);

    let art_image = Image::new();
    art_image.add_css_class("albumArt");
    art_image.set_pixel_size(80);
    art_image.set_icon_name(Some("audio-x-generic"));
    music_page.append(&art_image);

    let track_label = Label::new(Some("Not playing"));
    track_label.add_css_class("trackLabel");
    track_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    track_label.set_max_width_chars(22);
    music_page.append(&track_label);

    let artist_label = Label::new(None);
    artist_label.add_css_class("artistLabel");
    artist_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(22);
    music_page.append(&artist_label);

    let controls = GtkBox::new(Orientation::Horizontal, 6);
    controls.set_halign(gtk4::Align::Center);

    let prev_btn       = Button::with_label("⏮");
    let play_btn       = Button::with_label("▶");
    let next_track_btn = Button::with_label("⏭");

    for btn in &[&prev_btn, &play_btn, &next_track_btn] {
        btn.add_css_class("mediaBtn");
    }
    controls.append(&prev_btn);
    controls.append(&play_btn);
    controls.append(&next_track_btn);
    music_page.append(&controls);

    outer.append(&music_page);
    outer.append(&handle);

    win.set_child(Some(&outer));
    win.present();

    {
        let tl = track_label.clone();
        let al = artist_label.clone();
        let ai = art_image.clone();
        let pb = play_btn.clone();
        {
            let (tl2, al2, ai2, pb2) = (tl.clone(), al.clone(), ai.clone(), pb.clone());
            spawn_worker(fetch_music_state, move |s| apply_music_state(s, &tl2, &al2, &ai2, &pb2));
        }

        gtk4::glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            let (tl2, al2, ai2, pb2) = (tl.clone(), al.clone(), ai.clone(), pb.clone());
            spawn_worker(fetch_music_state, move |s| apply_music_state(s, &tl2, &al2, &ai2, &pb2));
            gtk4::glib::ControlFlow::Continue
        });
    }

    prev_btn.connect_clicked(|_| fire_playerctl(&["previous"]));
    next_track_btn.connect_clicked(|_| fire_playerctl(&["next"]));

    {
        let play_btn_c = play_btn.clone();
        play_btn.connect_clicked(move |btn| {
            fire_playerctl(&["play-pause"]);

            let now_playing = btn.label().map(|l| l == "▶").unwrap_or(false);
            play_btn_c.set_label(if now_playing { "⏸" } else { "▶" });

            let pb = play_btn_c.clone();
            gtk4::glib::timeout_add_local_once(
                std::time::Duration::from_millis(150),
                move || {
                    spawn_worker(
                        || pctl(&["status"]).map(|s| s == "Playing").unwrap_or(false),
                        move |playing| pb.set_label(if playing { "⏸" } else { "▶" }),
                    );
                },
            );
        });
    }

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));
    let gesture = gtk4::GestureDrag::new();

    {
        let outer_c  = music_page.clone();
        let handle_c = handle.clone();
        gesture.connect_drag_begin(move |_, _, _| {
            outer_c.add_css_class("jiggling");
            handle_c.set_cursor_from_name(Some("grabbing"));
        });
    }
    {
        let cx = cur_x.clone();
        let cy = cur_y.clone();
        let win_c = win.clone();
        gesture.connect_drag_update(move |_, dx, dy| {
            let nx = (cx.get() as f64 + dx).max(0.0) as i32;
            let ny = (cy.get() as f64 + dy).max(0.0) as i32;
            win_c.set_margin(Edge::Left, nx);
            win_c.set_margin(Edge::Top, ny);
        });
    }
    {
        let cx2 = cur_x.clone();
        let cy2 = cur_y.clone();
        let outer_c  = music_page.clone();
        let handle_c = handle.clone();
        let win_c    = win.clone();
        gesture.connect_drag_end(move |_, dx, dy| {
            handle_c.set_cursor_from_name(Some("grab"));
            let nx = (cx2.get() as f64 + dx).max(0.0) as i32;
            let ny = (cy2.get() as f64 + dy).max(0.0) as i32;
            cx2.set(nx);
            cy2.set(ny);
            win_c.set_margin(Edge::Left, nx);
            win_c.set_margin(Edge::Top, ny);
            save_position(NAME, nx, ny);
            outer_c.remove_css_class("jiggling");
        });
    }
    handle.add_controller(gesture);

    win
}

fn apply_music_state(
    state:        Option<MusicState>,
    track_label:  &Label,
    artist_label: &Label,
    art_image:    &Image,
    play_btn:     &Button,
) {
    match state {
        Some(s) => {
            track_label.set_label(&s.title);
            artist_label.set_label(&s.artist);
            play_btn.set_label(if s.playing { "⏸" } else { "▶" });
            if s.art_url.starts_with("file://") {
                art_image.set_from_file(Some(s.art_url.trim_start_matches("file://")));
            } else {
                art_image.set_icon_name(Some("audio-x-generic"));
            }
        }
        None => {
            track_label.set_label("Not playing");
            artist_label.set_label("");
            play_btn.set_label("▶");
            art_image.set_icon_name(Some("audio-x-generic"));
        }
    }
}