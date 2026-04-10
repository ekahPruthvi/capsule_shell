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

#[derive(Debug, Clone)]
pub enum OsdEvent {
    Volume   { volume: u32, muted: bool },
    Mute     { muted: bool, volume: u32 },
    MicMute  { muted: bool },
    MicInUse { active: bool },
}

#[derive(Default)]
struct AudioState {
    sink_volume: u32,
    sink_muted:  bool,
    src_muted:   bool,
    src_running: bool,
}

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

    {
        let ctx = Rc::clone(&context);
        let st  = Rc::clone(&state);
        let lbl = osd_label.clone();
        let rev = osd_revealer.clone();
        let hid = Rc::clone(&hide_id);

        context.borrow_mut().set_state_callback(Some(Box::new(move || {
            let cs = unsafe {
                (*ctx.as_ptr()).get_state()
            };

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

fn pa_vol_to_percent(v: Volume) -> u32 {
    let norm = Volume::NORMAL.0 as f64;
    ((v.0 as f64 / norm) * 100.0).round().clamp(0.0, 150.0) as u32
}

fn volume_bar(percent: u32) -> String {
    let filled = (percent.min(100) / 10) as usize;
    let empty  = 10usize.saturating_sub(filled);
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}