use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("desktopstat.sock")
    } else {
        PathBuf::from("/tmp/desktopstat.sock")
    }
}

fn encode(hiding: bool) -> &'static [u8] {
    if hiding { b"desktop\n" } else { b"appview\n" }
}

#[derive(Clone)]
pub struct HideBus {
    hiding: Arc<AtomicBool>,
    clients: Arc<Mutex<Vec<UnixStream>>>,
}

impl HideBus {
    pub fn spawn() -> Self {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);

        let bus = HideBus {
            hiding: Arc::new(AtomicBool::new(false)),
            clients: Arc::new(Mutex::new(Vec::new())),
        };

        match UnixListener::bind(&path) {
            Ok(listener) => {
                let bus_bg = bus.clone();
                thread::spawn(move || {
                    for conn in listener.incoming() {
                        let Ok(mut stream) = conn else { continue };

                        let state = bus_bg.hiding.load(Ordering::SeqCst);
                        if stream.write_all(encode(state)).is_err() {
                            continue;
                        }

                        let _ = stream.set_nonblocking(true);
                        bus_bg.clients.lock().unwrap().push(stream);
                    }
                });
            }
            Err(e) => {
                eprintln!("[desktopStat] couldn't bind {:?}: {e}", path);
            }
        }

        bus
    }

    pub fn get(&self) -> bool {
        self.hiding.load(Ordering::SeqCst)
    }

    pub fn set(&self, hiding: bool) {
        self.hiding.store(hiding, Ordering::SeqCst);
        let msg = encode(hiding);
        let mut clients = self.clients.lock().unwrap();
        clients.retain_mut(|c| match c.write_all(msg) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => true,
            Err(_) => false,
        });
    }
}

// natural language query foir humans ig (sounds cool)
pub fn nlq() -> i32 { 
    match UnixStream::connect(socket_path()) {
        Ok(stream) => {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    eprintln!("[desktopStat] no response from running instance");
                    1
                }
                Ok(_) => {
                    print!("{line}");
                    0
                }
            }
        }
        Err(e) => {
            eprintln!("[desktopStat] couldn't reach a running instance: {e}");
            1
        }
    }
}

// code based query for my modules inside capsule shell binary
pub fn cbq_ifdesktop() -> bool { 
    match UnixStream::connect(socket_path()) {
        Ok(stream) => {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    eprintln!("[desktopStat] no response from running instance");
                    return false;
                }
                Ok(_) => {
                    if line.trim() == "desktop" {
                        return true;
                    } else {
                        return false;
                    };
                }
            }
        }
        Err(e) => {
            eprintln!("[desktopStat] couldn't reach a running instance: {e}");
            return false;
        }
    }
}