use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, Orientation, Window, Image};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::Cell;
use std::process::Command;
use std::rc::Rc;
use std::thread;
use crate::widgets::position::{load_positions, save_position};

// i dont feel like renaming this from system to music but i have decided tht battery and other will be seperate and music will be seperate

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
    // music_overlay.set_margin_top(10);
    // music_overlay.set_margin_end(10);
    // music_overlay.set_margin_start(10);

    let music_page = GtkBox::new(Orientation::Vertical, 6);
    music_page.add_css_class("MusicWidget");

    use gtk4::gdk::prelude::GdkCairoContextExt as _;
    use gtk4::gdk_pixbuf::Pixbuf;
    use std::cell::RefCell;

    let art_pixbuf: Rc<RefCell<Option<Pixbuf>>> = Rc::new(RefCell::new(None));

    let art_canvas = gtk4::DrawingArea::new();
    art_canvas.set_content_width(200);
    art_canvas.set_content_height(200);
    art_canvas.add_css_class("albumArt");

    {
        let pb_ref = art_pixbuf.clone();
        art_canvas.set_draw_func(move |_w, cr, width, height| {
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

            let label_r = outer_r * 0.38;
            cr.set_line_width(0.6);
            let mut r = outer_r - 4.0;
            while r > label_r + 4.0 {
                cr.arc(cx, cy, r, 0.0, tau);
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
                let _ = cr.stroke();
                r -= 3.5;
            }

            let _ = cr.save();
            cr.arc(cx, cy, label_r, 0.0, tau);
            cr.clip();
            cr.new_path();
            match *pb_ref.borrow() {
                Some(ref pb) => {
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
            cr.set_operator(gtk4::cairo::Operator::Clear);
            let _ = cr.fill();
            cr.set_operator(gtk4::cairo::Operator::Over);

            cr.arc(cx, cy, outer_r - 1.5, 0.0, tau);
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.08);
            cr.set_line_width(2.5);
            let _ = cr.stroke();
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


    let prev_btn       = Button::builder().child(&prev).build();
    let play_btn       = Button::builder().child(&play).build();
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

    {
        let tl  = track_label.clone();
        let al  = artist_label.clone();
        let ac  = art_canvas.clone();
        let apb = art_pixbuf.clone();
        let pb  = play_btn.clone();
        let pi  = play.clone();
        let pai = pause.clone();
        let ipl = is_playing.clone();
        {
            let (tl2, al2, ac2, apb2, pb2, pi2, pai2, ipl2) =
                (tl.clone(), al.clone(), ac.clone(), apb.clone(),
                 pb.clone(), pi.clone(), pai.clone(), ipl.clone());
            spawn_worker(fetch_music_state, move |s| {
                apply_music_state(s, &tl2, &al2, &ac2, &apb2, &pb2, &pi2, &pai2, &ipl2)
            });
        }

        gtk4::glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            let (tl2, al2, ac2, apb2, pb2, pi2, pai2, ipl2) =
                (tl.clone(), al.clone(), ac.clone(), apb.clone(),
                 pb.clone(), pi.clone(), pai.clone(), ipl.clone());
            spawn_worker(fetch_music_state, move |s| {
                apply_music_state(s, &tl2, &al2, &ac2, &apb2, &pb2, &pi2, &pai2, &ipl2)
            });
            gtk4::glib::ControlFlow::Continue
        });
    }

    prev_btn.connect_clicked(|_| fire_playerctl(&["previous"]));
    next_track_btn.connect_clicked(|_| fire_playerctl(&["next"]));

    {
        let play_btn_c   = play_btn.clone();
        let play_img     = play.clone();
        let pause_img    = pause.clone();
        let is_playing_c = is_playing.clone();
        play_btn.connect_clicked(move |_btn| {
            fire_playerctl(&["play-pause"]);

            let now_playing = is_playing_c.get();
            is_playing_c.set(!now_playing);
            if !now_playing {
                play_btn_c.set_child(Some(&pause_img));
            } else {
                play_btn_c.set_child(Some(&play_img));
            }

            let pb            = play_btn_c.clone();
            let play_img2     = play_img.clone();
            let pause_img2    = pause_img.clone();
            let is_playing_c2 = is_playing_c.clone();
            gtk4::glib::timeout_add_local_once(
                std::time::Duration::from_millis(150),
                move || {
                    spawn_worker(
                        || pctl(&["status"]).map(|s| s == "Playing").unwrap_or(false),
                        move |playing| {
                            is_playing_c2.set(playing);
                            pb.set_child(Some(if playing { &pause_img2 } else { &play_img2 }));
                        },
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

fn fetch_pixbuf_from_url(url: &str) -> Option<gtk4::gdk_pixbuf::Pixbuf> {
    use gtk4::gdk_pixbuf::Pixbuf;
    use std::io::Read;

    let mut response = ureq::get(url).call().ok()?;
    let mut bytes = Vec::new();
    response.body_mut().as_reader().read_to_end(&mut bytes).ok()?;

    let stream = gtk4::gio::MemoryInputStream::from_bytes(&gtk4::glib::Bytes::from(&bytes));
    Pixbuf::from_stream(&stream, gtk4::gio::Cancellable::NONE).ok()
}

fn apply_music_state(
    state:        Option<MusicState>,
    track_label:  &Label,
    artist_label: &Label,
    art_canvas:   &gtk4::DrawingArea,
    art_pixbuf:   &Rc<std::cell::RefCell<Option<gtk4::gdk_pixbuf::Pixbuf>>>,
    play_btn:     &Button,
    play_img:     &Image,
    pause_img:    &Image,
    is_playing:   &Rc<Cell<bool>>,
) {
    use gtk4::gdk_pixbuf::Pixbuf;

    match state {
        Some(s) => {
            track_label.set_label(&s.title);
            artist_label.set_label(&s.artist);
            is_playing.set(s.playing);
            play_btn.set_child(Some(if s.playing { pause_img } else { play_img }));

            if s.playing {
                art_canvas.add_css_class("albumArtspinn");
            } else {
                art_canvas.remove_css_class("albumArtspinn");
            }

            let new_pb = if s.art_url.starts_with("file://") {
                let path = s.art_url.trim_start_matches("file://");
                Pixbuf::from_file(path).ok()
            } else if s.art_url.starts_with("http://") || s.art_url.starts_with("https://") {
                fetch_pixbuf_from_url(&s.art_url)
            } else {
                None
            };
            *art_pixbuf.borrow_mut() = new_pb;
            art_canvas.queue_draw();
        }
        None => {
            track_label.set_label("");
            artist_label.set_label("");
            is_playing.set(false);
            play_btn.set_child(Some(play_img));
            *art_pixbuf.borrow_mut() = None;
            art_canvas.queue_draw();
        }
    }
}