use libpulse_binding::{
    callbacks::ListResult,
    context::{
        subscribe::{Facility, InterestMaskSet, Operation as SubOp},
        Context, FlagSet as ContextFlagSet, State as ContextState,
    },
    volume::Volume,
};
use libpulse_glib_binding::Mainloop;

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

fn connect_brightness(
    rx:       std_mpsc::Receiver<u32>,
    label:    gtk4::Label,
    revealer: gtk4::Revealer,
    hide_id:  Rc<RefCell<Option<glib::SourceId>>>,
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
            show_osd(&label, &revealer, &hide_id, OsdEvent::Brightness { percent: pct });
        }
        glib::ControlFlow::Continue
    });
}

pub fn connect_osd_to_dock(
    osd_label:    &gtk4::Label,
    osd_revealer: &gtk4::Revealer,
) {
    if let Some(bright_rx) = spawn_brightness_watcher() {
        connect_brightness(
            bright_rx,
            osd_label.clone(),
            osd_revealer.clone(),
            Rc::new(RefCell::new(None)),
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

    let state :Rc<RefCell<AudioState>> = Default::default();
    let audio_hide_id :Rc<RefCell<Option<glib::SourceId>>> = Default::default();

    {
        let ctx = Rc::clone(&context);
        let st  = Rc::clone(&state);
        let lbl = osd_label.clone();
        let rev = osd_revealer.clone();
        let hid = Rc::clone(&audio_hide_id);

        context.borrow_mut().set_state_callback(Some(Box::new(move || {
            let cs = unsafe { (*ctx.as_ptr()).get_state() };
            match cs {
                ContextState::Ready => {
                    let ctx2 = Rc::clone(&ctx);
                    let st2  = Rc::clone(&st);
                    let lbl2 = lbl.clone();
                    let rev2 = rev.clone();
                    let hid2 = Rc::clone(&hid);
                    glib::idle_add_local_once(move || {
                        on_context_ready(&ctx2, &st2, &lbl2, &rev2, &hid2);
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

fn on_context_ready(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    label:    &gtk4::Label,
    revealer: &gtk4::Revealer,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
) {
    fetch_sink_info(ctx, state, label, revealer, hide_id, false);
    fetch_source_info(ctx, state, label, revealer, hide_id, false);

    ctx.borrow_mut()
        .subscribe(InterestMaskSet::SINK | InterestMaskSet::SOURCE, |_| {});

    {
        let ctx2 = Rc::clone(ctx);
        let st2  = Rc::clone(state);
        let lbl2 = label.clone();
        let rev2 = revealer.clone();
        let hid2 = Rc::clone(hide_id);

        ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
            move |facility, op, _index| {
                match (facility, op) {
                    (Some(Facility::Sink), Some(SubOp::Changed)) => {
                        fetch_sink_info(&ctx2, &st2, &lbl2, &rev2, &hid2, true);
                    }
                    (Some(Facility::Source), Some(SubOp::Changed)) => {
                        fetch_source_info(&ctx2, &st2, &lbl2, &rev2, &hid2, true);
                    }
                    _ => {}
                }
            },
        )));
    }
}

fn fetch_sink_info(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    label:    &gtk4::Label,
    revealer: &gtk4::Revealer,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    emit:     bool,
) {
    let st  = Rc::clone(state);
    let lbl = label.clone();
    let rev = revealer.clone();
    let hid = Rc::clone(hide_id);

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
            show_osd(&lbl, &rev, &hid, OsdEvent::Mute { muted, volume: vol });
        } else if vol_changed {
            show_osd(&lbl, &rev, &hid, OsdEvent::Volume { volume: vol, muted });
        }
    });
}

fn fetch_source_info(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    label:    &gtk4::Label,
    revealer: &gtk4::Revealer,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    emit:     bool,
) {
    let st  = Rc::clone(state);
    let lbl = label.clone();
    let rev = revealer.clone();
    let hid = Rc::clone(hide_id);

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
            show_osd(&lbl, &rev, &hid, OsdEvent::MicMute { muted });
        } else if running_changed && running {
            show_osd(&lbl, &rev, &hid, OsdEvent::MicInUse { active: true });
        }
    });
}

fn show_osd(
    label:    &gtk4::Label,
    revealer: &gtk4::Revealer,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
    event:    OsdEvent,
) {
    apply_osd_event(label, &event);
    revealer.set_reveal_child(true);

    if let Some(id) = hide_id.borrow_mut().take() {
        id.remove();
    }
    let rev = revealer.clone();
    let hid = Rc::clone(hide_id);
    let new_id = glib::timeout_add_seconds_local(2, move || {
        rev.set_reveal_child(false);
        hid.borrow_mut().take();
        glib::ControlFlow::Break
    });
    *hide_id.borrow_mut() = Some(new_id);
}

fn apply_osd_event(label: &gtk4::Label, event: &OsdEvent) {
    for cls in &["osd-volume", "osd-muted", "osd-mic", "osd-mic-active", "osd-brightness"] {
        label.remove_css_class(cls);
    }

    match event {
        OsdEvent::Volume { volume, .. } => {
            label.set_text(&format!("󰕾  {}%  {}", volume, block_bar(*volume)));
            label.add_css_class("osd-volume");
        }
        OsdEvent::Mute { muted: true, .. } => {
            label.set_text("󰖁  muted");
            label.add_css_class("osd-muted");
        }
        OsdEvent::Mute { muted: false, volume } => {
            label.set_text(&format!("󰕾  {}%  {}", volume, block_bar(*volume)));
            label.add_css_class("osd-volume");
        }
        OsdEvent::MicMute { muted: true } => {
            label.set_text("󰍭  mic muted");
            label.add_css_class("osd-mic");
        }
        OsdEvent::MicMute { muted: false } => {
            label.set_text("󰍬  mic on");
            label.add_css_class("osd-mic");
        }
        OsdEvent::MicInUse { active: true } => {
            label.set_text("󰍬  mic in use");
            label.add_css_class("osd-mic-active");
        }
        OsdEvent::MicInUse { active: false } => {
            return;
        }
        OsdEvent::Brightness { percent } => {
            let icon = if *percent >= 50 { "󰃠" } else { "󰃟" };
            label.set_text(&format!("{}  {}%  {}", icon, percent, block_bar(*percent)));
            label.add_css_class("osd-brightness");
        }
    }
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn pa_vol_to_percent(v: Volume) -> u32 {
    let norm = Volume::NORMAL.0 as f64;
    ((v.0 as f64 / norm) * 100.0).round().clamp(0.0, 150.0) as u32
}

fn block_bar(percent: u32) -> String {
    let filled = (percent.min(100) / 10) as usize;
    let empty  = 10usize.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}