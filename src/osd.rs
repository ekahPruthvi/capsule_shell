use libpulse_binding::{
    callbacks::ListResult,
    context::{
        subscribe::{Facility, InterestMaskSet, Operation as SubOp},
        Context, FlagSet as ContextFlagSet, State as ContextState,
    },
    volume::Volume,
};
use libpulse_glib_binding::Mainloop;
use std::process::Command;

use gtk4::glib;
use gtk4::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc as std_mpsc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum OsdEvent {
    Volume     { volume: u32, muted: bool },
    Mute       { muted: bool, volume: u32 },
    MicMute    { muted: bool },
    MicInUse   { active: bool },
    Brightness { percent: u32 },
}

#[derive(Default)]
struct AudioState {
    sink_volume: u32,
    sink_muted:  bool,
    src_muted:   bool,
    src_running: bool,
}

fn adjust_volume(delta: f64) {
    let arg = if delta < 0.0 { "+5%" } else { "-5%" };
    let _ = Command::new("pactl")
        .args(["set-sink-volume", "@DEFAULT_SINK@", arg])
        .spawn();
}

fn adjust_brightness(delta: f64) {
    let arg = if delta < 0.0 { "+5%" } else { "5%-" };
    let _ = Command::new("brightnessctl")
        .args(["set", arg])
        .spawn();
}

// ─── backlight ────────────────────────────────────────────────────────────────

fn find_backlight() -> Option<(std::path::PathBuf, u64)> {
    let dir = std::fs::read_dir("/sys/class/backlight").ok()?;
    for entry in dir.flatten() {
        let base = entry.path();
        let max_path = base.join("max_brightness");
        let cur_path = base.join("brightness");
        if cur_path.exists() && max_path.exists() {
            let max: u64 = std::fs::read_to_string(&max_path)
                .ok()?
                .trim()
                .parse()
                .ok()?;
            return Some((cur_path, max));
        }
    }
    None
}

fn read_brightness_percent(path: &std::path::Path, max: u64) -> Option<u32> {
    let cur: u64 = std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse()
        .ok()?;
    Some(((cur * 100 / max.max(1)) as u32).min(100))
}

fn spawn_brightness_watcher() -> Option<std_mpsc::Receiver<u32>> {
    let (path, max) = find_backlight()?;
    let (tx, rx) = std_mpsc::channel::<u32>();

    std::thread::spawn(move || {
        use inotify::{Inotify, WatchMask};

        let mut inotify = match Inotify::init() {
            Ok(i) => i,
            Err(e) => { eprintln!("[osd] inotify init failed: {e}"); return; }
        };

        if let Err(e) = inotify.watches().add(&path, WatchMask::CLOSE_WRITE | WatchMask::MODIFY) {
            eprintln!("[osd] inotify watch failed on {}: {e}", path.display());
            return;
        }

        let mut last: Option<u32> = None;
        let mut buf = [0u8; 512];

        loop {
            match inotify.read_events_blocking(&mut buf) {
                Ok(_) => {
                    if let Some(pct) = read_brightness_percent(&path, max) {
                        if last != Some(pct) {
                            last = Some(pct);
                            let _ = tx.send(pct);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[osd] inotify read error: {e}");
                    break;
                }
            }
        }
    });

    Some(rx)
}

// ─── brightness connector ─────────────────────────────────────────────────────

fn connect_brightness(
    rx:       std_mpsc::Receiver<u32>,
    osd_box:  gtk4::Box,
    revealer: gtk4::Revealer,
    capsule:  gtk4::Box,
    window:   gtk4::ApplicationWindow,
    hide_id:  Rc<RefCell<Option<glib::SourceId>>>,
    osd_label: gtk4::Label,
) {
    let rx = Rc::new(RefCell::new(rx));

    glib::timeout_add_local(Duration::from_millis(50), move || {
        let mut last_pct = None;
        loop {
            match rx.borrow().try_recv() {
                Ok(pct) => { last_pct = Some(pct); }
                Err(std_mpsc::TryRecvError::Empty) => break,
                Err(std_mpsc::TryRecvError::Disconnected) => {
                    return glib::ControlFlow::Break;
                }
            }
        }
        if let Some(pct) = last_pct {
            show_osd(
                &osd_box, &revealer, &capsule, &window,
                &hide_id, OsdEvent::Brightness { percent: pct },
                &osd_label,
            );
        }
        glib::ControlFlow::Continue
    });
}

// ─── public entry point ───────────────────────────────────────────────────────

pub fn connect_osd_to_dock(
    osd_box:      &gtk4::Box,
    osd_revealer: &gtk4::Revealer,
    capsule:      &gtk4::Box,
    window:       &gtk4::ApplicationWindow,
    osd_label:    &gtk4::Label,
) {
    let is_volume_mode = true;

    let scrl_pad = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .hexpand(true)
        .halign(gtk4::Align::Fill)
        .vexpand(false)
        .valign(gtk4::Align::Baseline)
        .height_request(10)
        .width_request(900)
        .css_classes(["scrollPad"])
        .build();

    let scroll_controller = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);

    scroll_controller.connect_scroll(move |_, _dx, dy| {
        if is_volume_mode {
            adjust_volume(dy);
        } else {
            adjust_brightness(dy);
        }
        glib::Propagation::Proceed
    });

    scrl_pad.add_controller(scroll_controller);

    capsule.append(&scrl_pad);

    if let Some(bright_rx) = spawn_brightness_watcher() {
        connect_brightness(
            bright_rx,
            osd_box.clone(),
            osd_revealer.clone(),
            capsule.clone(),
            window.clone(),
            Rc::new(RefCell::new(None)),
            osd_label.clone(),
        );
    }

    let mainloop = Rc::new(RefCell::new(
        Mainloop::new(None).expect("PA glib mainloop"),
    ));

    let context = {
        let ml = mainloop.borrow();
        Rc::new(RefCell::new(
            Context::new(&*ml, "capsule-osd").expect("PA context"),
        ))
    };

    let state:         Rc<RefCell<AudioState>>             = Default::default();
    let audio_hide_id: Rc<RefCell<Option<glib::SourceId>>> = Default::default();

    {
        let ctx = Rc::clone(&context);
        let st  = Rc::clone(&state);
        let bx  = osd_box.clone();
        let rev = osd_revealer.clone();
        let cap = capsule.clone();
        let win = window.clone();
        let hid = Rc::clone(&audio_hide_id);
        let osxout = osd_label.clone();

        context.borrow_mut().set_state_callback(Some(Box::new(move || {
            let cs = unsafe { (*ctx.as_ptr()).get_state() };
            match cs {
                ContextState::Ready => {
                    let ctx2 = Rc::clone(&ctx);
                    let st2  = Rc::clone(&st);
                    let bx2  = bx.clone();
                    let rev2 = rev.clone();
                    let cap2 = cap.clone();
                    let win2 = win.clone();
                    let hid2 = Rc::clone(&hid);
                    let osxout2 = osxout.clone();
                    glib::idle_add_local_once(move || {
                        on_context_ready(&ctx2, &st2, &bx2, &rev2, &cap2, &win2, &hid2, &osxout2);
                    });
                }
                ContextState::Failed | ContextState::Terminated => {
                    eprintln!("[osd] PA context failed/terminated");
                }
                _ => {}
            }
        })));
    }

    context
        .borrow_mut()
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .expect("PA connect");

    std::mem::forget((mainloop, context));
}

// ─── context ready ────────────────────────────────────────────────────────────

fn on_context_ready(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    osd_box:  &gtk4::Box,
    revealer: &gtk4::Revealer,
    capsule:  &gtk4::Box,
    window:   &gtk4::ApplicationWindow,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    osd_label: &gtk4::Label,
) {
    fetch_sink_info(ctx, state, osd_box, revealer, capsule, window, hide_id, false, osd_label);
    fetch_source_info(ctx, state, osd_box, revealer, capsule, window, hide_id, false, osd_label);

    ctx.borrow_mut()
        .subscribe(InterestMaskSet::SINK | InterestMaskSet::SOURCE, |_| {});

    {
        let ctx2 = Rc::clone(ctx);
        let st2  = Rc::clone(state);
        let bx2  = osd_box.clone();
        let rev2 = revealer.clone();
        let cap2 = capsule.clone();
        let win2 = window.clone();
        let hid2 = Rc::clone(hide_id);
        let osxout2= osd_label.clone();

        ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
            move |facility, op, _index| {
                match (facility, op) {
                    (Some(Facility::Sink), Some(SubOp::Changed)) => {
                        fetch_sink_info(&ctx2, &st2, &bx2, &rev2, &cap2, &win2, &hid2, true, &osxout2);
                    }
                    (Some(Facility::Source), Some(SubOp::Changed)) => {
                        fetch_source_info(&ctx2, &st2, &bx2, &rev2, &cap2, &win2, &hid2, true, &osxout2);
                    }
                    _ => {}
                }
            },
        )));
    }
}

// ─── sink / source fetchers ───────────────────────────────────────────────────

fn fetch_sink_info(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    osd_box:  &gtk4::Box,
    revealer: &gtk4::Revealer,
    capsule:  &gtk4::Box,
    window:   &gtk4::ApplicationWindow,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    emit:     bool,
    osd_label: &gtk4::Label,
) {
    let st  = Rc::clone(state);
    let bx  = osd_box.clone();
    let rev = revealer.clone();
    let cap = capsule.clone();
    let win = window.clone();
    let hid = Rc::clone(hide_id);
    let osxout = osd_label.clone();

    let introspector = ctx.borrow().introspect();
    let _ = introspector.get_sink_info_by_name("@DEFAULT_SINK@", move |res| {
        let ListResult::Item(info) = res else { return };

        let vol   = pa_vol_to_percent(info.volume.avg());
        let muted = info.mute;

        let mut s = st.borrow_mut();
        let vol_changed  = s.sink_volume != vol;
        let mute_changed = s.sink_muted  != muted;
        s.sink_volume = vol;
        s.sink_muted  = muted;
        drop(s);

        if !emit { return; }
        if mute_changed {
            show_osd(&bx, &rev, &cap, &win, &hid, OsdEvent::Mute { muted, volume: vol }, &osxout);
        } else if vol_changed {
            show_osd(&bx, &rev, &cap, &win, &hid, OsdEvent::Volume { volume: vol, muted }, &osxout);
        }
    });
}

fn fetch_source_info(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    osd_box:  &gtk4::Box,
    revealer: &gtk4::Revealer,
    capsule:  &gtk4::Box,
    window:   &gtk4::ApplicationWindow,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    emit:     bool,
    osd_label: &gtk4::Label,
) {
    let st  = Rc::clone(state);
    let bx  = osd_box.clone();
    let rev = revealer.clone();
    let cap = capsule.clone();
    let win = window.clone();
    let hid = Rc::clone(hide_id);
    let osxout = osd_label.clone();

    let introspector = ctx.borrow().introspect();
    let _ = introspector.get_source_info_by_name("@DEFAULT_SOURCE@", move |res| {
        let ListResult::Item(info) = res else { return };

        let muted   = info.mute;
        let running = matches!(
            info.state,
            libpulse_binding::def::SourceState::Running
        );

        let mut s = st.borrow_mut();
        let mute_changed    = s.src_muted   != muted;
        let running_changed = s.src_running != running;
        s.src_muted   = muted;
        s.src_running = running;
        drop(s);

        if !emit { return; }
        if mute_changed {
            show_osd(&bx, &rev, &cap, &win, &hid, OsdEvent::MicMute { muted }, &osxout);
        } else if running_changed && running {
            show_osd(&bx, &rev, &cap, &win, &hid, OsdEvent::MicInUse { active: true }, &osxout);
        }
    });
}

// ─── show / apply ─────────────────────────────────────────────────────────────

fn show_osd(
    osd_box:  &gtk4::Box,
    revealer: &gtk4::Revealer,
    capsule:  &gtk4::Box,
    window:   &gtk4::ApplicationWindow,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    event:    OsdEvent,
    osd_label: &gtk4::Label,
) {
    apply_osd_event(osd_box, &event, osd_label);

    let already_open = revealer.reveals_child();
    revealer.set_reveal_child(true);


    let rev = revealer.clone();
    let cap = capsule.clone();
    if !already_open {
        window.present();
        cap.add_css_class("osd-show");
        glib::timeout_add_local(Duration::from_millis(300), move || {      
            cap.remove_css_class("osd-show");
            glib::ControlFlow::Break
        });
        rev.set_visible(true);
    }

    if let Some(id) = hide_id.borrow_mut().take() {
        id.remove();
    }

    let rev = revealer.clone();
    let cap = capsule.clone();
    let win = window.clone();
    let hid = Rc::clone(hide_id);

    let new_id = glib::timeout_add_seconds_local(3, move || {
        cap.add_css_class("osd-hide");
        let rev = rev.clone();
        let cap = cap.clone();
        let hid = hid.clone();
        let win = win.clone();
        glib::timeout_add_local(Duration::from_millis(300), move || {      
            cap.remove_css_class("osd-hide");
            rev.set_reveal_child(false);
            rev.set_visible(false);
            hid.borrow_mut().take();
            win.hide();
            glib::ControlFlow::Break
        });
        glib::ControlFlow::Break
    });

    *hide_id.borrow_mut() = Some(new_id);
}

fn apply_osd_event(osd_box: &gtk4::Box, event: &OsdEvent, osd_label: &gtk4::Label) {
    let total_width = 300;

    for cls in &["osd-volume", "osd-muted", "osd-mic", "osd-mic-active", "osd-brightness"] {
        osd_box.remove_css_class(cls);
    }

    while let Some(child) = osd_box.first_child() {
        osd_box.remove(&child);
    }
    
    osd_label.set_text("");

    match event {
        OsdEvent::Volume { volume, muted: false } => {
            let fill = ((total_width as f64) * (*volume as f64 / 100.0)) as i32;
            osd_box.set_width_request(fill.max(4));
            osd_box.add_css_class("osd-volume");
            osd_label.set_text(&format!("{}", volume));        }
        OsdEvent::Volume { muted: true, .. } | OsdEvent::Mute { muted: true, .. } => {
            osd_box.set_width_request(total_width);
            osd_box.add_css_class("osd-muted");
            osd_label.set_text(&format!("Volume muted"));
        }
        OsdEvent::Mute { muted: false, volume } => {
            let fill = ((total_width as f64) * (*volume as f64 / 100.0)) as i32;
            osd_box.set_width_request(fill.max(4));
            osd_box.add_css_class("osd-volume");
            osd_label.set_text(&format!("Volume: {}", volume));
        }
        OsdEvent::MicMute { muted: true } => {
            osd_box.set_width_request(total_width);
            osd_box.add_css_class("osd-muted");
            osd_label.set_text(&format!("Mic muted"));
        }
        OsdEvent::MicMute { muted: false } => {
            osd_box.set_width_request(total_width);
            osd_box.add_css_class("osd-mic");
            osd_label.set_text(&format!("Mic on"));
        }
        OsdEvent::MicInUse { active: true } => {
            osd_box.set_width_request(total_width);
            osd_box.add_css_class("osd-mic");
            osd_label.set_text(&format!("Mic in use"));
        }
        OsdEvent::MicInUse { active: false } => {
            osd_box.set_width_request(total_width);
            osd_box.add_css_class("osd-mic");
            osd_label.set_text(&format!("Mic not in use"));
        }
        OsdEvent::Brightness { percent } => {
            let fill = ((total_width as f64) * (*percent as f64 / 100.0)) as i32;
            osd_box.set_width_request(fill.max(4));
            osd_box.add_css_class("osd-brightness");
            osd_label.set_text(&format!("{}", percent));
        }
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn pa_vol_to_percent(v: Volume) -> u32 {
    let norm = Volume::NORMAL.0 as f64;
    ((v.0 as f64 / norm) * 100.0).round().clamp(0.0, 150.0) as u32
}