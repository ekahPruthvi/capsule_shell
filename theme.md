# Capsule Shell Theme Reference

This page catalogs CSS class names found in `config/capsule/light.css` and `config/capsule/dark.css`, and indicates where Rust assigns or references them. CSS is the source of truth for visual behavior; Rust is responsible for attaching classes to GTK widgets.

## Stylesheet entry points

- `config/capsule/light.css` — light appearance rules.
- `config/capsule/dark.css` — dark appearance rules.

## Component and class catalogue

### `.MuicInfo`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.MusicWidget`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.PercentLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.albumArtspinn`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ampm`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs, src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.appName`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.appdBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.appdMenu`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.artistLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.batBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.batLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.batring`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.blight`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.blip`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.clearBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.clippy`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.clippyFileBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.closeBox`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.cos`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.cosIcon`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs, src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ctrlBTNSbox`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ctrlBackdrop`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ctrlBtnL`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ctrlBtnS`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.date`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.day-name`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.day-number`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.deleteBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dndicon`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dockBox`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dockBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs, src/ctrl.rs, src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dockBtnActive`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dockcum`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs, src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dockleave`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs, src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dragHandle`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs, src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dragHandleM`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/calendar.rs, src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dragHandlestick`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/stick.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.dummytxt`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.errWidget`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.filled`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.flyplane`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.handleNextBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.horizontal`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.jiggling`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs, src/widgets/battery.rs, src/widgets/calendar.rs, src/widgets/stick.rs, src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.mediaBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.month`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.muted`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netBtnBody`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netBtnLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netList`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netListEmpty`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netListRowBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netListScroll`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netPanel`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.netPanelBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.nooclip`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notPlaying`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.noti-critical`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.noti-low`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.noti-normal`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notiCapsule`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notiIcon`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notiPopup`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notiScroller`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notificationAll`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notificationAllLabelBody`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notificationAllLabelSummary`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notificationWindow`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notification_badge`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notification_btn`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.notification_heading_button`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.okWidget`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-brightness`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-fill-box`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-hide`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-mic`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-muted`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-show`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osd-volume`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osdBox`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osdCapsule`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.osdLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.other-month`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.outerAppd`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/appd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.outerBat`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/battery.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.outerSys`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.qlbar`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.qlicons`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.scale-in`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.scale-out`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.scrollPad`

- **Defined in:** `dark.css`
- **Rust references:** src/osd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.spinning`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.spinning-coin`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdBar`

- **Defined in:** `dark.css`
- **Rust references:** src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdClose`

- **Defined in:** `dark.css`
- **Rust references:** src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdFloat`

- **Defined in:** `dark.css`
- **Rust references:** src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdMin`

- **Defined in:** `dark.css`
- **Rust references:** src/ssd.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.ssdMinActive`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.starting`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs, src/notifications.rs, src/widgets/battery.rs, src/widgets/calendar.rs, src/widgets/stick.rs, src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.startingOSD`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.statusicon`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.sticker_img`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.tNa`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.time`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.timeCapsule`

- **Defined in:** `dark.css`
- **Rust references:** src/main.rs, src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.today`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.trackLabel`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/system.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.trayBtn`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.trayMenuBox`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.trayMenuItem`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.trayMenuSeparator`

- **Defined in:** `dark.css`
- **Rust references:** src/altdock.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.userHello`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.userName`

- **Defined in:** `dark.css`
- **Rust references:** src/ctrl.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.vanish`

- **Defined in:** `dark.css`
- **Rust references:** src/notifications.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.vertical`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.view`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.vol`

- **Defined in:** `light.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.week-number`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.widgetBox`

- **Defined in:** `dark.css`
- **Rust references:** src/widgets/calendar.rs
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

### `.year`

- **Defined in:** `dark.css`
- **Rust references:** No direct Rust string reference detected
- **Purpose:** Styling hook for a GTK widget, state, layout container, or interaction state. Inspect the matching selector for exact colors, dimensions, transitions, and pseudo-state rules.

## Common state classes

The project uses several classes as runtime state markers. Their exact visual effect is defined by the stylesheet:

- `jiggling` — applied during widget dragging.
- `muted` — used for muted audio state where referenced.
- `filled` — used by level/progress styling where referenced.
- `notPlaying` — music widget idle state.
- `osd-show` / `osd-hide` — OSD visibility states.

## Adding a new component

1. Give the widget a stable semantic class name.
2. Add the class to both light and dark stylesheets when the component supports both themes.
3. Attach the class from Rust using `add_css_class` or `set_css_classes`.
4. Document the class in this file and link the implementing Rust module.
5. Keep interaction/state classes separate from structural classes so animations and visual states remain maintainable.

## Related documentation

- [Project README](./README.md)
- [Rust module documentation](./docs/)