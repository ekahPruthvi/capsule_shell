// osd.rs — volume / mute / mic OSD via libpulse-binding.
//
// Cargo.toml:
//   libpulse-binding      = "2"
//   libpulse-glib-binding = "2"
//
// The key borrow-safety rule with libpulse + RefCell:
//   Never hold a borrow on the RefCell while a PA callback that also borrows
//   it can fire. We enforce this by:
//   1. Deferring on_context_ready via glib::idle_add_local_once so the
//      state-callback's implicit borrow on `ctx` is fully off the stack.
//   2. In every fetch_* function, we call .introspect() and immediately drop
//      the ctx borrow BEFORE handing control to PA — the result callback then
//      borrows ctx fresh with no contention.

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

// ─── public event type ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum OsdEvent {
    Volume   { volume: u32, muted: bool },
    Mute     { muted: bool, volume: u32 },
    MicMute  { muted: bool },
    MicInUse { active: bool },
}

// ─── internal state ───────────────────────────────────────────────────────────

#[derive(Default)]
struct AudioState {
    sink_volume: u32,
    sink_muted:  bool,
    src_muted:   bool,
    src_running: bool,
}

// ─── entry point ──────────────────────────────────────────────────────────────

pub fn connect_osd_to_dock(
    osd_label:    &gtk4::Label,
    osd_revealer: &gtk4::Revealer,
) {
    let mainloop = Rc::new(RefCell::new(
        Mainloop::new(None).expect("PA glib mainloop"),
    ));

    let context = {
        let ml = mainloop.borrow();
        Rc::new(RefCell::new(
            Context::new(&*ml, "capsule-osd").expect("PA context"),
        ))
    };

    let state:   Rc<RefCell<AudioState>>          = Default::default();
    let hide_id: Rc<RefCell<Option<glib::SourceId>>> = Default::default();

    // ── state callback ───────────────────────────────────────────────────────
    // IMPORTANT: the closure must not borrow `context` via the Rc while the
    // callback itself is being invoked — PA holds an internal lock during the
    // callback. We clone all Rcs before registering and defer the Ready work
    // with idle_add_local_once so the callback stack is fully unwound first.
    {
        let ctx = Rc::clone(&context);
        let st  = Rc::clone(&state);
        let lbl = osd_label.clone();
        let rev = osd_revealer.clone();
        let hid = Rc::clone(&hide_id);

        // borrow_mut for set_state_callback — dropped at end of this block.
        context.borrow_mut().set_state_callback(Some(Box::new(move || {
            // Read state with a *shared* borrow — doesn't conflict with PA's lock.
            let cs = unsafe {
                // Safety: we are on the single glib main thread; no other
                // Rust code can concurrently touch the RefCell here.
                (*ctx.as_ptr()).get_state()
            };

            match cs {
                ContextState::Ready => {
                    // Defer: yield back to glib so this callback's implicit
                    // PA-internal borrow is fully released before we start
                    // calling subscribe / introspect on the context.
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
    } // borrow_mut dropped here

    // connect() borrows context mutably for the duration of the call only.
    context
        .borrow_mut()
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .expect("PA connect");

    // Leak both into 'static — the dock lives for the process lifetime.
    std::mem::forget((mainloop, context));
}

// ─── ready handler (runs on next glib idle, clean call stack) ─────────────────

fn on_context_ready(
    ctx:      &Rc<RefCell<Context>>,
    state:    &Rc<RefCell<AudioState>>,
    label:    &gtk4::Label,
    revealer: &gtk4::Revealer,
    hide_id:  &Rc<RefCell<Option<glib::SourceId>>>,
) {
    // Populate baseline state — no OSD shown.
    fetch_sink_info(ctx, state, label, revealer, hide_id, false);
    fetch_source_info(ctx, state, label, revealer, hide_id, false);

    // subscribe() — brief mut borrow, immediately dropped.
    ctx.borrow_mut()
        .subscribe(InterestMaskSet::SINK | InterestMaskSet::SOURCE, |_| {});

    // set_subscribe_callback — brief mut borrow, immediately dropped.
    {
        let ctx2 = Rc::clone(ctx);
        let st2  = Rc::clone(state);
        let lbl2 = label.clone();
        let rev2 = revealer.clone();
        let hid2 = Rc::clone(hide_id);

        ctx.borrow_mut().set_subscribe_callback(Some(Box::new(
            move |facility, op, _index| {
                // This callback fires on the glib main thread.
                // fetch_* functions borrow ctx only briefly then release
                // before handing off to PA — no re-entrant borrow possible.
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
    } // borrow_mut dropped
}

// ─── fetchers ─────────────────────────────────────────────────────────────────
// Pattern: borrow ctx → call introspect() → immediately drop borrow.
// The introspect() call returns an Introspector by value; PA uses it
// asynchronously, calling our closure later on the glib thread — at which
// point ctx's RefCell is no longer borrowed.

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

    // Borrow, get introspector (cheap handle), drop borrow.
    let introspector = ctx.borrow().introspect();
    // `introspector` is now owned; ctx borrow is released.
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

// ─── OSD display ──────────────────────────────────────────────────────────────

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
    let new_id = glib::timeout_add_seconds_local(2, move || {
        rev.set_reveal_child(false);
        glib::ControlFlow::Break
    });
    *hide_id.borrow_mut() = Some(new_id);
}

fn apply_osd_event(label: &gtk4::Label, event: &OsdEvent) {
    let text = match event {
        OsdEvent::Volume { volume, .. } =>
            format!("󰕾  {}%  {}", volume, volume_bar(*volume)),
        OsdEvent::Mute { muted: true, .. } =>
            "󰖁  muted".to_string(),
        OsdEvent::Mute { muted: false, volume } =>
            format!("󰕾  {}%  {}", volume, volume_bar(*volume)),
        OsdEvent::MicMute { muted: true }  => "󰍭  mic muted".to_string(),
        OsdEvent::MicMute { muted: false } => "󰍬  mic on".to_string(),
        OsdEvent::MicInUse { active: true }  => "󰍬  mic in use".to_string(),
        OsdEvent::MicInUse { active: false } => return,
    };
    label.set_text(&text);

    for cls in &["osd-volume", "osd-muted", "osd-mic", "osd-mic-active"] {
        label.remove_css_class(cls);
    }
    let cls = match event {
        OsdEvent::Volume { .. }              => "osd-volume",
        OsdEvent::Mute { muted: false, .. }  => "osd-volume",
        OsdEvent::Mute { muted: true, .. }   => "osd-muted",
        OsdEvent::MicMute { .. }             => "osd-mic",
        OsdEvent::MicInUse { active: true }  => "osd-mic-active",
        OsdEvent::MicInUse { active: false } => return,
    };
    label.add_css_class(cls);
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn pa_vol_to_percent(v: Volume) -> u32 {
    let norm = Volume::NORMAL.0 as f64;
    ((v.0 as f64 / norm) * 100.0).round().clamp(0.0, 150.0) as u32
}

fn volume_bar(percent: u32) -> String {
    let filled = (percent.min(100) / 10) as usize;
    let empty  = 10usize.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}