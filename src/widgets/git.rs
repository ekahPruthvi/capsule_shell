use gtk4::prelude::*;
use gtk4::{Box as GtkBox, DrawingArea, Label, Orientation, Window};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::thread;
use std::time::{Duration, Instant};
use crate::widgets::position::{load_positions, save_position};

const NAME: &str = "git";
const PROBE_PATH: &str = "/var/lib/cynager/info.probe";

// Heatmap geometry. GitHub returns ~53 weeks; we only draw the newest WEEKS_SHOWN.
const WEEKS_SHOWN: usize = 20;
const CELL: f64 = 10.0;
const GAP: f64 = 3.0;

const REFRESH_SECS: u64 = 30 * 60;
const TICK_SECS: u32 = 60;

const QUERY: &str = "query { viewer { login contributionsCollection { contributionCalendar { \
totalContributions weeks { contributionDays { contributionCount contributionLevel } } } } } }";

// ───────────────────────── data ─────────────────────────

#[derive(Clone, Debug)]
struct Day {
    count: u32,
    level: u8,
}

#[derive(Clone, Debug)]
enum GitState {
    Loading,
    NoToken,
    Error(String),
    Ok {
        login: String,
        total: u64,
        weeks: Vec<Vec<Day>>,
    },
}

impl GitState {
    fn is_ok(&self) -> bool {
        matches!(self, GitState::Ok { .. })
    }
}

fn read_token() -> Option<String> {
    let content = std::fs::read_to_string(PROBE_PATH).ok()?;
    let set_start = content.find(":set")?;
    let set_body = &content[set_start + 4..];
    let set_end = set_body.find(":end")?;
    let mut body = set_body[..set_end].to_string();

    if let Some(ws) = body.find("widgets") {
        if let Some(open) = body[ws..].find('{') {
            if let Some(close) = body[ws + open..].find('}') {
                body.replace_range(ws..ws + open + close + 1, "");
            }
        }
    }

    for line in body.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("git") {
            if let Some(val) = rest.trim().strip_prefix(':') {
                let tok = val.trim().trim_matches(|c| c == '"' || c == '\'');
                if !tok.is_empty() {
                    return Some(tok.to_string());
                }
            }
        }
    }
    None
}

fn level_from(s: &str) -> u8 {
    match s {
        "FIRST_QUARTILE" => 1,
        "SECOND_QUARTILE" => 2,
        "THIRD_QUARTILE" => 3,
        "FOURTH_QUARTILE" => 4,
        _ => 0,
    }
}

fn parse_response(v: &serde_json::Value) -> Result<GitState, String> {
    if let Some(errs) = v.get("errors").and_then(|e| e.as_array()) {
        let msg = errs
            .first()
            .and_then(|e| e["message"].as_str())
            .unwrap_or("GraphQL error");
        return Err(msg.to_string());
    }
    let viewer = &v["data"]["viewer"];
    let login = viewer["login"].as_str().unwrap_or("").to_string();
    let cal = &viewer["contributionsCollection"]["contributionCalendar"];
    let total = cal["totalContributions"]
        .as_u64()
        .ok_or_else(|| "unexpected response".to_string())?;
    let weeks = cal["weeks"]
        .as_array()
        .ok_or_else(|| "unexpected response".to_string())?
        .iter()
        .map(|w| {
            w["contributionDays"]
                .as_array()
                .map(|days| {
                    days.iter()
                        .map(|d| Day {
                            count: d["contributionCount"].as_u64().unwrap_or(0) as u32,
                            level: level_from(d["contributionLevel"].as_str().unwrap_or("NONE")),
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();
    Ok(GitState::Ok { login, total, weeks })
}

fn fetch_contributions() -> GitState {
    let Some(token) = read_token() else {
        return GitState::NoToken;
    };

    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(15)))
            .build(),
    );

    let payload = serde_json::json!({ "query": QUERY }).to_string();

    let resp = agent
        .post("https://api.github.com/graphql")
        .header("Authorization", &format!("bearer {token}"))
        .header("User-Agent", "capsule-shell")
        .header("Content-Type", "application/json")
        .send(payload.as_str());

    match resp {
        Ok(mut r) => match r
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).map_err(|e| e.to_string()))
        {
            Ok(v) => parse_response(&v).unwrap_or_else(GitState::Error),
            Err(e) => GitState::Error(format!("bad response: {e}")),
        },
        Err(ureq::Error::StatusCode(401)) => GitState::Error("token rejected (401)".into()),
        Err(ureq::Error::StatusCode(403)) => GitState::Error("forbidden / rate limited (403)".into()),
        Err(ureq::Error::StatusCode(code)) => GitState::Error(format!("http {code}")),
        Err(_) => GitState::Error("offline".into()),
    }
}

fn fetch_async(on_done: impl FnOnce(GitState) + 'static) {
    let (tx, rx) = std::sync::mpsc::channel::<GitState>();
    thread::spawn(move || {
        let _ = tx.send(fetch_contributions());
    });
    let mut on_done = Some(on_done);
    gtk4::glib::timeout_add_local(Duration::from_millis(150), move || match rx.try_recv() {
        Ok(state) => {
            if let Some(f) = on_done.take() {
                f(state);
            }
            gtk4::glib::ControlFlow::Break
        }
        Err(TryRecvError::Empty) => gtk4::glib::ControlFlow::Continue,
        Err(TryRecvError::Disconnected) => gtk4::glib::ControlFlow::Break,
    })
    ;
}

// ───────────────────────── drawing ─────────────────────────

fn rounded_rect(cr: &gtk4::cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    use std::f64::consts::{FRAC_PI_2, PI};
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, FRAC_PI_2, PI);
    cr.arc(x + r, y + r, r, PI, 3.0 * FRAC_PI_2);
    cr.close_path();
}

fn level_color(level: u8) -> (f64, f64, f64, f64) {
    match level {
        0 => (0.22, 0.22, 0.24, 0.6),
        1 => (0.055, 0.267, 0.161, 1.0),
        2 => (0.0, 0.427, 0.196, 1.0),
        3 => (0.149, 0.651, 0.255, 1.0),
        _ => (0.224, 0.827, 0.325, 1.0),
    }
}

fn make_heatmap(state: Rc<RefCell<GitState>>) -> DrawingArea {
    let da = DrawingArea::new();
    da.set_content_width((WEEKS_SHOWN as f64 * (CELL + GAP) - GAP) as i32);
    da.set_content_height((7.0 * (CELL + GAP) - GAP) as i32);

    da.set_draw_func(move |_, cr, _w, _h| {
        let st = state.borrow();
        let weeks: Vec<Vec<Day>> = match &*st {
            GitState::Ok { weeks, .. } => {
                let skip = weeks.len().saturating_sub(WEEKS_SHOWN);
                weeks.iter().skip(skip).cloned().collect()
            }
            _ => (0..WEEKS_SHOWN)
                .map(|_| (0..7).map(|_| Day { count: 0, level: 0 }).collect())
                .collect(),
        };

        let x_off = (WEEKS_SHOWN - weeks.len().min(WEEKS_SHOWN)) as f64 * (CELL + GAP);

        for (wi, week) in weeks.iter().enumerate() {
            for (di, day) in week.iter().enumerate() {
                let x = x_off + wi as f64 * (CELL + GAP);
                let y = di as f64 * (CELL + GAP);
                let (r, g, b, a) = level_color(day.level);
                rounded_rect(cr, x, y, CELL, CELL, 2.5);
                cr.set_source_rgba(r, g, b, a);
                let _ = cr.fill();
            }
        }
    });

    da
}

fn apply_state(state: &GitState, total: &Label, sub: &Label, da: &DrawingArea) {
    match state {
        GitState::Loading => {
            total.set_label("…");
            sub.set_label("loading");
        }
        GitState::NoToken => {
            total.set_label("no token");
            sub.set_label("set  git :<token>  in info.probe");
        }
        GitState::Error(msg) => {
            total.set_label("error");
            sub.set_label(msg);
        }
        GitState::Ok { login, total: n, .. } => {
            total.set_label(&n.to_string());
            sub.set_label(&format!("{login}"));
        }
    }
    da.queue_draw();
}

// ───────────────────────── widget ─────────────────────────

pub fn spawn_git_widget(monitor: Option<&gtk4::gdk::Monitor>) -> Window {
    let positions = load_positions();
    let (start_x, start_y) = positions.get(NAME).copied().unwrap_or((40, 400));

    let win = Window::builder().title("capsuleGit").build();
    win.init_layer_shell();
    win.set_layer(Layer::Bottom);
    win.set_namespace(Some("cosWidget"));
    win.set_anchor(Edge::Top, true);
    win.set_anchor(Edge::Left, true);
    win.set_exclusive_zone(0);
    win.set_margin(Edge::Top, start_y);
    win.set_margin(Edge::Left, start_x);
    if let Some(m) = monitor {
        win.set_monitor(Some(m));
    }
    win.remove_css_class("background");
    win.add_css_class("gitpage");

    let outer = GtkBox::new(Orientation::Vertical, 0);
    outer.set_css_classes(&["starting", "outerGit"]);

    let git_page = GtkBox::new(Orientation::Vertical, 8);
    git_page.set_hexpand(true);
    git_page.set_vexpand(true);

    let header = GtkBox::new(Orientation::Horizontal, 0);
    header.set_hexpand(true);

    let titles = GtkBox::new(Orientation::Vertical, 0);
    titles.set_hexpand(true);
    titles.set_halign(gtk4::Align::Start);

    let total_label = Label::new(Some("…"));
    total_label.add_css_class("gitLabel");
    total_label.set_halign(gtk4::Align::Start);

    let sub_label = Label::new(Some("loading"));
    sub_label.add_css_class("gitSub");
    sub_label.set_halign(gtk4::Align::Start);

    titles.append(&sub_label);

    let handle = GtkBox::new(Orientation::Horizontal, 0);
    handle.add_css_class("dragHandle");
    handle.set_cursor_from_name(Some("grab"));
    handle.set_width_request(100);
    handle.set_hexpand(true);
    handle.set_margin_start(10);
    handle.set_margin_end(10);
    handle.set_halign(gtk4::Align::Fill);
    handle.set_valign(gtk4::Align::Start);

    // header.append(&titles);
    header.append(&handle);

    let state: Rc<RefCell<GitState>> = Rc::new(RefCell::new(GitState::Loading));
    let heatmap = make_heatmap(state.clone());
    heatmap.add_css_class("gitHeatmap");
    heatmap.set_halign(gtk4::Align::Start);

    git_page.append(&header);
    git_page.append(&heatmap);
    outer.append(&git_page);

    win.set_child(Some(&outer));
    win.present();

    {
        let busy = Rc::new(Cell::new(false));
        let last_ok: Rc<Cell<Option<Instant>>> = Rc::new(Cell::new(None));

        let state_c = state.clone();
        let total_c = total_label.clone();
        let sub_c = sub_label.clone();
        let da_c = heatmap.clone();

        let tick: Rc<dyn Fn()> = Rc::new(move || {
            if busy.get() {
                return;
            }
            let fresh = last_ok
                .get()
                .map_or(false, |t| t.elapsed() < Duration::from_secs(REFRESH_SECS));
            if fresh {
                return;
            }
            busy.set(true);

            let busy = busy.clone();
            let last_ok = last_ok.clone();
            let state = state_c.clone();
            let total = total_c.clone();
            let sub = sub_c.clone();
            let da = da_c.clone();
            fetch_async(move |new_state| {
                if new_state.is_ok() {
                    last_ok.set(Some(Instant::now()));
                }
                let keep_old = matches!(new_state, GitState::Error(_)) && state.borrow().is_ok();
                if !keep_old {
                    *state.borrow_mut() = new_state;
                }
                apply_state(&state.borrow(), &total, &sub, &da);
                busy.set(false);
            });
        });

        tick();
        let tick2 = tick.clone();
        gtk4::glib::timeout_add_seconds_local(TICK_SECS, move || {
            tick2();
            gtk4::glib::ControlFlow::Continue
        });
    }

    let cur_x = Rc::new(Cell::new(start_x));
    let cur_y = Rc::new(Cell::new(start_y));
    let gesture = gtk4::GestureDrag::new();

    {
        let page_c = git_page.clone();
        let handle_c = handle.clone();
        gesture.connect_drag_begin(move |_, _, _| {
            page_c.add_css_class("jiggling");
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
        let page_c = git_page.clone();
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
            page_c.remove_css_class("jiggling");
        });
    }
    handle.add_controller(gesture);

    win
}