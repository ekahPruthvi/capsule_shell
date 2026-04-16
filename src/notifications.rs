use zbus::connection::Builder;
use tokio::sync::mpsc;
use gtk4::{ApplicationWindow, Image, Label, prelude::*, Box as GtkBox, glib};
use gtk4::glib::clone;
use tokio::sync::mpsc::UnboundedReceiver;
use std::cell::{RefCell, Cell};
use std::rc::Rc;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub icon: String,
    pub timestamp: std::time::Instant,
    pub actions: Vec<String>
}

struct NotificationServer {
    sender: mpsc::UnboundedSender<Notification>,
    next_id: std::sync::atomic::AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    async fn notify(
        &self,
        app_name: &str,
        _replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        _hints: std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
        _expire_timeout: i32,
    ) -> u32 {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let notif = Notification {
            id,
            app_name: app_name.to_string(),
            summary: summary.to_string(),
            body: body.to_string(),
            icon: app_icon.to_string(),
            timestamp: std::time::Instant::now(),
            actions: actions,
        };

        let _ = self.sender.send(notif);
        id
    }

    async fn get_capabilities(&self) -> Vec<String> {
        vec!["body".into(), "persistence".into()]
    }

    async fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("capsule", "ekah", "1.0", "1.2")
    }

    async fn close_notification(&self, _id: u32) {}
}

pub fn spawn_messaging_daemon() -> UnboundedReceiver<Notification> {
    let (tx, rx) = mpsc::unbounded_channel();

    std::thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async move {
                let server = NotificationServer {
                    sender: tx,
                    next_id: std::sync::atomic::AtomicU32::new(1),
                };

                let _conn = Builder::session()
                    .expect("dbus session")
                    .name("org.freedesktop.Notifications")
                    .expect("dbus name")
                    .serve_at("/org/freedesktop/Notifications", server)
                    .expect("serve_at")
                    .build()
                    .await
                    .expect("dbus connection");

                std::future::pending::<()>().await;
            });
    });

    rx
}

pub fn connect_notifications_to_dock(
    mut rx: UnboundedReceiver<Notification>,
    noti_window: &GtkBox,
    main_window: &ApplicationWindow,
    app_img: &Image,
    badge: &Label // gotta add time box for displaying dot to show unread notifications
) {
    let history: Rc<RefCell<VecDeque<Notification>>> =
        Rc::new(RefCell::new(VecDeque::with_capacity(50)));

    let ctx = gtk4::glib::MainContext::default();
    ctx.spawn_local(clone!(
        #[strong] noti_window,
        #[strong] main_window,
        #[strong] app_img,
        #[strong] badge,
        async move {
            while let Some(notif) = rx.recv().await {
                {
                    let mut h = history.borrow_mut();
                    if h.len() == 50 {
                        h.pop_front();
                    }
                    h.push_back(notif.clone());
                    badge.set_visible(true);
                    app_img.set_from_file(Some(&notif.icon));
                    let display = gtk4::gdk::Display::default().expect("Could not get default display");
                    let monitors = display.monitors();
                    let main_window = main_window.clone();
                    if let Some(monitor) = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>() {
                        let geometry = monitor.geometry();
                        let width = geometry.width();
                        let requested_width = (width as f64 * 0.8) as i32;
                        let target_width = requested_width; 
                        let start_width = 300;
                        let duration_ms = 1500.0;
                        let fps = 60.0;
                        let increment_per_frame = (target_width - start_width) as f64 / (duration_ms / (1000.0 / fps));

                        let current_width = Rc::new(Cell::new(start_width as f64));

                        main_window.set_width_request(requested_width+10);

                        noti_window.set_width_request(start_width);

                        let noti_window = noti_window.clone();
                        let badge = badge.clone();

                        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                            let next_w = current_width.get() + increment_per_frame;
                            
                            if next_w >= target_width as f64 {
                                noti_window.set_width_request(target_width);
                                noti_window.set_css_classes(&["blip","timeCapsule"]);
                                badge.set_text(&format!("{}\n{}", notif.summary, notif.body));

                                let noti_window_inner = noti_window.clone();
                                let badge_inner = badge.clone();
                                let current_width_inner = current_width.clone();
                                let main_inner = main_window.clone();
                                
                                glib::timeout_add_local(std::time::Duration::from_millis(5000), move || {
                                    let noti_window_c = noti_window_inner.clone();
                                    let current_width_c = current_width_inner.clone();
                                    let main_c = main_inner.clone();
                                    badge_inner.set_text("");
                                    glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
                                        let current_w = current_width_c.get();
                                        let next_w = current_w - increment_per_frame;

                                        if next_w <= start_width as f64 {
                                            noti_window_c.set_width_request(start_width);
                                            noti_window_c.remove_css_class("blip");
                                            main_c.set_width_request(300);
                                            return glib::ControlFlow::Break;
                                        }

                                        current_width_c.set(next_w);
                                        noti_window_c.set_width_request(next_w as i32);
                                        glib::ControlFlow::Continue
                                    });
                                    glib::ControlFlow::Break // End the 5s delay timer
                                });
                                return gtk4::glib::ControlFlow::Break;
                            }
                            
                            current_width.set(next_w);
                            noti_window.set_width_request(next_w as i32);
                            gtk4::glib::ControlFlow::Continue
                        });

                        // noti_window.set_width_request(requested_width);
                    }
                }

                let count = history.borrow().len();
            }
        }
    ));
}