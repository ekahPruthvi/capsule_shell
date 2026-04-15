use zbus::connection::Builder; // zbus 4.x — ConnectionBuilder is deprecated
use tokio::sync::mpsc;
use gtk4::{ApplicationWindow, Image, Label, prelude::*};
use gtk4::glib::clone;
use tokio::sync::mpsc::UnboundedReceiver;
use std::cell::RefCell;
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
        _actions: Vec<String>,
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
    noti_window: &ApplicationWindow,
    app_img: &Image,
    badge: &Label // gotta add time box for displaying dot to show unread notifications
) {
    let history: Rc<RefCell<VecDeque<Notification>>> =
        Rc::new(RefCell::new(VecDeque::with_capacity(50)));

    let ctx = gtk4::glib::MainContext::default();
    ctx.spawn_local(clone!(
        #[strong] noti_window,
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
                    badge.set_text(&format!("{}\n{}", notif.summary, notif.icon));
                }

                let count = history.borrow().len();
            }
        }
    ));
}