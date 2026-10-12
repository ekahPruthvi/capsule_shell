use crate::widgets::position::{load_positions, save_position};
use gtk4::cairo;
use gtk4::gdk::prelude::GdkCairoContextExt as _;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use gtk4::{Box as GtkBox, Button, DrawingArea, Image, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::io::Read;
use std::process::Command;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

// i dont feel like renaming this from system to music but i have decided tht battery and other will be seperate and music will be seperate

const NAME: &str = "system";

/// The label on the vinyl is only ~76 logical px wide, so there is no point
/// keeping a 1000px cover in memory. 160 covers HiDPI (2x) comfortably.
const ART_DECODE_PX: i32 = 160;

/// Field separator for `playerctl --format` (ASCII unit separator).
const SEP: char = '\u{1f}';

// ───────────────────────────── data ─────────────────────────────

enum ArtUpdate {
    /// art_url is the same as last time, keep what we have
    Unchanged,
    /// new raw image bytes (decoded later on the GTK thread, at small size)
    Set(Vec<u8>),
    /// no art / failed to load
    Clear,
}

struct MusicState {
    title: String,
    artist: String,
    art_url: String,
    playing: bool,
    art: ArtUpdate,
}

/// Everything the poll/apply code needs. All fields are cheap to clone (refcounts).
#[derive(Clone)]
struct MusicUi {
    track: Label,
    artist: Label,
    canvas: DrawingArea,
    play_btn: Button,
    play_img: Image,
    pause_img: Image,
    is_playing: Rc<Cell<bool>>,
    pixbuf: Rc<RefCell<Option<Pixbuf>>>,
    /// pre-rendered vinyl; None = needs re-render
    vinyl_cache: Rc<RefCell<Option<cairo::ImageSurface>>>,
    last_art_url: Rc<RefCell<String>>,
    /// a fetch is already running, skip this tick
    busy: Rc<Cell<bool>>,
}

// ───────────────────────── playerctl (worker thread only) ─────────────────────────

fn pctl(args: &[&str]) -> Option<String> {
    let out = Command::new("playerctl").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn load_art_bytes(url: &str) -> Option<Vec<u8>> {
    if url.starts_with("file://") {
        // gio handles percent-encoding (%20 etc.)
        let path = gio::File::for_uri(url).path()?;
        return std::fs::read(path).ok();
    }
    if url.starts_with("http://") || url.starts_with("https://") {
        let mut resp = ureq::get(url)
            .config()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .call()
            .ok()?;
        let mut bytes = Vec::new();
        resp.body_mut()
            .as_reader()
            .take(8 * 1024 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        return Some(bytes);
    }
    None
}

/// ONE playerctl process instead of ~7, and all file/network IO happens here,
/// off the GTK thread.
fn fetch_music_state(last_art_url: String) -> Option<MusicState> {
    let fmt = format!(
        "{{{{playerName}}}}{s}{{{{status}}}}{s}{{{{title}}}}{s}{{{{artist}}}}{s}{{{{mpris:artUrl}}}}",
        s = SEP
    );
    let raw = pctl(&["--all-players", "metadata", "--format", &fmt])?;

    let mut first: Option<Vec<&str>> = None;
    let mut chosen: Option<Vec<&str>> = None;
    for line in raw.lines() {
        let f: Vec<&str> = line.split(SEP).collect();
        if f.len() < 5 {
            continue;
        }
        if f[1] == "Playing" {
            chosen = Some(f);
            break;
        }
        if first.is_none() {
            first = Some(f);
        }
    }
    let f = chosen.or(first)?;
    let (status, title, artist, art_url) = (f[1], f[2], f[3], f[4]);

    if status == "Stopped" || status.is_empty() || title.is_empty() {
        return None;
    }

    let art = if art_url == last_art_url {
        ArtUpdate::Unchanged
    } else {
        match load_art_bytes(art_url) {
            Some(b) => ArtUpdate::Set(b),
            None => ArtUpdate::Clear,
        }
    };

    Some(MusicState {
        title: title.to_owned(),
        artist: artist.to_owned(),
        art_url: art_url.to_owned(),
        playing: status == "Playing",
        art,
    })
}

fn fire_playerctl(args: &'static [&'static str]) {
    thread::spawn(move || {
        let _ = Command::new("playerctl").args(args).status();
    });
}

/// Run `work` on a thread-pool thread, then `on_done` on the GTK main thread.
/// (No more idle-callback busy loop that spins the main loop while waiting.)
fn spawn_worker<T, W, D>(work: W, on_done: D)
where
    T: Send + 'static,
    W: FnOnce() -> T + Send + 'static,
    D: FnOnce(T) + 'static,
{
    glib::spawn_future_local(async move {
        if let Ok(v) = gio::spawn_blocking(work).await {
            on_done(v);
        }
    });
}

// ───────────────────────── drawing ─────────────────────────

/// Draw the whole vinyl ONCE into an offscreen surface. The widget's draw func
/// just paints this surface, so rotating/redrawing is nearly free.
fn render_vinyl(width: i32, height: i32, scale: i32, art: Option<&Pixbuf>) -> Option<cairo::ImageSurface> {
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width * scale, height * scale).ok()?;
    surface.set_device_scale(scale as f64, scale as f64);

    {
        let cr = cairo::Context::new(&surface).ok()?;
        let w = width as f64;
        let h = height as f64;
        let cx = w / 2.0;
        let cy = h / 2.0;
        let outer_r = cx.min(cy);
        let tau = 2.0 * std::f64::consts::PI;

        cr.arc(cx, cy, outer_r, 0.0, tau);
        cr.clip();
        cr.new_path();

        cr.arc(cx, cy, outer_r, 0.0, tau);
        cr.set_source_rgb(0.08, 0.08, 0.08);
        let _ = cr.fill();

        // grooves: one path, one stroke (was ~18 separate strokes)
        let label_r = outer_r * 0.38;
        cr.set_line_width(0.6);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
        let mut r = outer_r - 4.0;
        while r > label_r + 4.0 {
            cr.new_sub_path();
            cr.arc(cx, cy, r, 0.0, tau);
            r -= 3.5;
        }
        let _ = cr.stroke();

        // label / album art
        let _ = cr.save();
        cr.arc(cx, cy, label_r, 0.0, tau);
        cr.clip();
        cr.new_path();
        match art {
            Some(pb) => {
                let diameter = label_r * 2.0;
                let sx = diameter / pb.width() as f64;
                let sy = diameter / pb.height() as f64;
                cr.translate(cx - label_r, cy - label_r);
                cr.scale(sx, sy);
                cr.set_source_pixbuf(pb, 0.0, 0.0);
                let _ = cr.paint();
            }
            None => {
                cr.set_source_rgb(0.22, 0.10, 0.10);
                let _ = cr.paint();
            }
        }
        let _ = cr.restore();

        cr.arc(cx, cy, label_r, 0.0, tau);
        cr.set_source_rgba(0.0, 0.0, 0.0, 0.4);
        cr.set_line_width(1.5);
        let _ = cr.stroke();

        let hole_r = outer_r * 0.04;
        cr.arc(cx, cy, hole_r, 0.0, tau);
        cr.set_operator(cairo::Operator::Clear);
        let _ = cr.fill();
        cr.set_operator(cairo::Operator::Over);

        cr.arc(cx, cy, outer_r - 1.5, 0.0, tau);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
        cr.set_line_width(2.5);
        let _ = cr.stroke();
    }

    Some(surface)
}

fn decode_art(bytes: &[u8]) -> Option<Pixbuf> {
    let stream = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(bytes));
    // decode straight to a small size instead of loading e.g. 1000x1000 and
    // rescaling it in cairo on every draw
    Pixbuf::from_stream_at_scale(&stream, ART_DECODE_PX, ART_DECODE_PX, false, gio::Cancellable::NONE).ok()
}

fn set_class(w: &impl IsA<gtk4::Widget>, class: &str, on: bool) {
    if w.has_css_class(class) != on {
        if on {
            w.add_css_class(class);
        } else {
            w.remove_css_class(class);
        }
    }
}

// ───────────────────────── polling / applying ─────────────────────────

fn poll_music(ui: &MusicUi) {
    if ui.busy.get() {
        return; // previous fetch still running
    }
    ui.busy.set(true);

    let ui = ui.clone();
    let last = ui.last_art_url.borrow().clone();
    glib::spawn_future_local(async move {
        let res = gio::spawn_blocking(move || fetch_music_state(last)).await;
        ui.busy.set(false);
        if let Ok(state) = res {
            apply_music_state(state, &ui);
        }
    });
}

fn apply_music_state(state: Option<MusicState>, ui: &MusicUi) {
    match state {
        Some(s) => {
            ui.track.set_label(&s.title);
            ui.artist.set_label(&s.artist);

            // only touch the button when something actually changed
            if ui.is_playing.get() != s.playing {
                ui.is_playing.set(s.playing);
                ui.play_btn
                    .set_child(Some(if s.playing { &ui.pause_img } else { &ui.play_img }));
            }
            set_class(&ui.canvas, "albumArtspinn", s.playing);

            match s.art {
                ArtUpdate::Unchanged => {}
                ArtUpdate::Set(bytes) => {
                    *ui.pixbuf.borrow_mut() = decode_art(&bytes);
                    *ui.vinyl_cache.borrow_mut() = None;
                    ui.canvas.queue_draw();
                }
                ArtUpdate::Clear => {
                    *ui.pixbuf.borrow_mut() = None;
                    *ui.vinyl_cache.borrow_mut() = None;
                    ui.canvas.queue_draw();
                }
            }
            *ui.last_art_url.borrow_mut() = s.art_url;
        }
        None => {
            ui.track.set_label("");
            ui.artist.set_label("");
            if ui.is_playing.get() {
                ui.is_playing.set(false);
                ui.play_btn.set_child(Some(&ui.play_img));
            }
            set_class(&ui.canvas, "albumArtspinn", false);

            if ui.pixbuf.borrow().is_some() {
                *ui.pixbuf.borrow_mut() = None;
                *ui.vinyl_cache.borrow_mut() = None;
                ui.canvas.queue_draw();
            }
            ui.last_art_url.borrow_mut().clear();
        }
    }
}

// ───────────────────────── widget ─────────────────────────

pub fn spawn_sys_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 160));

    let win = Window::builder().title("capsuleWmus").build();
    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    win.remove_css_class("background");
    if let Some(m) = monitor {
        win.set_monitor(Some(m));
    }

    let outer = GtkBox::new(Orientation::Horizontal, 5);
    outer.set_css_classes(&["starting", "outerSys"]);
    outer.set_width_request(200);
    outer.set_height_request(200);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandleM");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_margin_top(60);
    handle.set_margin_start(10);
    handle.set_margin_end(3);
    handle.set_margin_bottom(10);
    handle.set_width_request(10);
    handle.set_vexpand(true);

    let music_overlay = gtk4::Overlay::new();

    let music_page = GtkBox::new(Orientation::Vertical, 6);
    music_page.add_css_class("MusicWidget");

    let art_pixbuf: Rc<RefCell<Option<Pixbuf>>> = Rc::new(RefCell::new(None));
    let vinyl_cache: Rc<RefCell<Option<cairo::ImageSurface>>> = Rc::new(RefCell::new(None));

    let art_canvas = DrawingArea::new();
    art_canvas.set_content_width(200);
    art_canvas.set_content_height(200);
    art_canvas.add_css_class("albumArt");

    {
        let pb_ref = art_pixbuf.clone();
        let cache = vinyl_cache.clone();
        art_canvas.set_draw_func(move |widget, cr, width, height| {
            let scale = widget.scale_factor().max(1);
            let mut c = cache.borrow_mut();

            let stale = match c.as_ref() {
                Some(s) => s.width() != width * scale || s.height() != height * scale,
                None => true,
            };
            if stale {
                *c = render_vinyl(width, height, scale, pb_ref.borrow().as_ref());
            }
            if let Some(s) = c.as_ref() {
                let _ = cr.set_source_surface(s, 0.0, 0.0);
                let _ = cr.paint();
            }
        });
    }

    music_overlay.set_child(Some(&art_canvas));
    music_overlay.add_overlay(&music_page);

    let track_label = Label::new(Some(""));
    track_label.add_css_class("trackLabel");
    track_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    track_label.set_max_width_chars(22);
    track_label.set_halign(gtk4::Align::Start);
    track_label.set_margin_start(10);
    track_label.set_margin_end(20);

    let artist_label = Label::new(Some(""));
    artist_label.add_css_class("artistLabel");
    artist_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    artist_label.set_max_width_chars(22);
    artist_label.set_halign(gtk4::Align::Start);
    artist_label.set_margin_start(10);
    artist_label.set_margin_end(20);

    let info = GtkBox::new(Orientation::Vertical, 0);
    info.add_css_class("MuicInfo");
    info.set_width_request(120);
    info.set_hexpand(false);
    info.set_halign(gtk4::Align::Start);
    info.append(&track_label);
    info.append(&artist_label);

    let controls = GtkBox::new(Orientation::Vertical, 6);
    controls.set_valign(gtk4::Align::Center);

    let play: Image = Image::from_file("/var/lib/cynager/icons/transperent.svg");
    play.set_icon_size(gtk4::IconSize::Large);
    let pause = Image::from_file("/var/lib/cynager/icons/pause.svg");
    pause.set_icon_size(gtk4::IconSize::Large);
    let prev = Image::from_file("/var/lib/cynager/icons/baw.svg");
    prev.set_icon_size(gtk4::IconSize::Normal);
    let next = Image::from_file("/var/lib/cynager/icons/fow.svg");
    next.set_icon_size(gtk4::IconSize::Normal);

    let prev_btn = Button::builder().child(&prev).build();
    let play_btn = Button::builder().child(&play).build();
    play_btn.set_cursor_from_name(Some("pointer"));
    let next_track_btn = Button::builder().child(&next).build();

    for btn in &[&prev_btn, &play_btn, &next_track_btn] {
        btn.add_css_class("mediaBtn");
    }

    let spacer = GtkBox::new(Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    spacer.set_valign(gtk4::Align::Baseline);

    music_page.append(&spacer);

    // controls.append(&prev_btn);
    controls.append(&play_btn);
    // controls.append(&next_track_btn);

    music_page.append(&controls);
    music_page.append(&info);

    outer.append(&music_overlay);
    outer.append(&handle);

    win.set_child(Some(&outer));
    win.present();

    let is_playing: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    let ui = MusicUi {
        track: track_label.clone(),
        artist: artist_label.clone(),
        canvas: art_canvas.clone(),
        play_btn: play_btn.clone(),
        play_img: play.clone(),
        pause_img: pause.clone(),
        is_playing: is_playing.clone(),
        pixbuf: art_pixbuf.clone(),
        vinyl_cache: vinyl_cache.clone(),
        last_art_url: Rc::new(RefCell::new(String::new())),
        busy: Rc::new(Cell::new(false)),
    };

    poll_music(&ui);
    {
        let ui = ui.clone();
        glib::timeout_add_local(Duration::from_secs(2), move || {
            poll_music(&ui);
            glib::ControlFlow::Continue
        });
    }

    prev_btn.connect_clicked(|_| fire_playerctl(&["previous"]));
    next_track_btn.connect_clicked(|_| fire_playerctl(&["next"]));

    {
        let play_btn_c = play_btn.clone();
        let play_img = play.clone();
        let pause_img = pause.clone();
        let is_playing_c = is_playing.clone();
        let canvas_c = art_canvas.clone();
        play_btn.connect_clicked(move |_btn| {
            fire_playerctl(&["play-pause"]);

            let now_playing = is_playing_c.get();
            is_playing_c.set(!now_playing);
            if !now_playing {
                play_btn_c.set_child(Some(&pause_img));
            } else {
                play_btn_c.set_child(Some(&play_img));
            }
            set_class(&canvas_c, "albumArtspinn", !now_playing);

            let pb = play_btn_c.clone();
            let play_img2 = play_img.clone();
            let pause_img2 = pause_img.clone();
            let is_playing_c2 = is_playing_c.clone();
            let canvas_c2 = canvas_c.clone();
            glib::timeout_add_local_once(Duration::from_millis(150), move || {
                spawn_worker(
                    || pctl(&["status"]).map(|s| s == "Playing").unwrap_or(false),
                    move |playing| {
                        is_playing_c2.set(playing);
                        pb.set_child(Some(if playing { &pause_img2 } else { &play_img2 }));
                        set_class(&canvas_c2, "albumArtspinn", playing);
                    },
                );
            });
        });
    }

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));
    let gesture = gtk4::GestureDrag::new();

    {
        let outer_c = music_page.clone();
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
        let outer_c = music_page.clone();
        let handle_c = handle.clone();
        let win_c = win.clone();
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