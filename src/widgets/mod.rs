use gtk4::prelude::*;
use gtk4::Window;

pub mod system;
pub mod calendar;
pub mod position;

pub fn kill(win: &Window) {
    win.close();
}
