use gtk4::{
    glib, prelude::*, Box as GtkBox, Button, Entry, Grid, Label, Orientation, Revealer, GestureClick
};
use std::env;
use std::fs::{self, File };
use std::io::{self, Read, Write};
use std::path::PathBuf;

fn cal_matrix(box_width: u32, box_height: u32) -> (u32, u32) {
    let margin = 80;
    let spacing = 30;
    let block_size = 200;

    let available_width = box_width.saturating_sub(margin * 2);
    let available_height = box_height.saturating_sub(margin * 2);

    let max_columns = if available_width + spacing >= block_size + spacing {
        (available_width + spacing) / (block_size + spacing)
    } else {
        0
    };

    let max_rows = if available_height + spacing >= block_size + spacing {
        (available_height + spacing) / (block_size + spacing)
    } else {
        0
    };

    (max_rows, max_columns)
}

fn config_control() -> io::Result<String> {
    let home_dir = env::var("HOME").map(PathBuf::from).expect("HOME env not set");

    let path: PathBuf = home_dir
        .join(".config")
        .join("capsule")
        .join("desktop")
        .join("widgets.dat");

    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    if !path.exists() {
        let mut file = File::create(&path)?;
        file.write_all(b"fill")?;
        return Ok("fill".to_string());
    }

    let mut contents = String::new();
    File::open(&path)?.read_to_string(&mut contents)?;
    Ok(contents)
}

pub fn build(mainbox :GtkBox) {
    let dummy = GtkBox::new(Orientation::Horizontal, 0);
    dummy.append(&Label::new(Some("dummy")));
    dummy.set_size_request(100, 80);
    dummy.set_widget_name("dummy");

    let grid = Grid::new();
    grid.set_column_spacing(30);
    grid.set_row_spacing(30);
    grid.set_margin_start(80);
    grid.set_margin_end(80);
    grid.set_hexpand(true);
    grid.set_vexpand(true);
    grid.set_halign(gtk4::Align::Center);

    let mainbox_clone = mainbox.clone();
    let grid_clone = grid.clone();
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        let (rows, cols) = cal_matrix(mainbox_clone.width().try_into().unwrap(), mainbox_clone.height().try_into().unwrap());
        match config_control() {
            Ok(config) => {
                let grid_clone_inner = grid_clone.clone();
                for line in config.lines() {
                    let grid_clone_inner2 = grid_clone_inner.clone();
                    if line.contains("fill") {
                        let mut count = 1;
                        // let skip_indices = [6, 7, 13, 14];

                        for row in 0..rows {
                            for col in 0..cols {
                                // if skip_indices.contains(&count) {
                                //     count+= 1;
                                // } else {
                                    let button = Button::with_label(&format!("{}", count));
                                    button.set_size_request(200, 200);
                                    grid_clone_inner2.attach(&button, col as i32, row as i32, 1, 1);
                                    count += 1;
                                // }
                            }
                        }

                        // let button = Button::with_label(&format!("{}", count));
                        // button.set_size_request(400, 400);
                        // grid_clone.attach(&button, 5, 0, 2, 2);       
                    }
                    if line.starts_with("file|") {
                    let parts: Vec<&str> = line.split('|').collect();
                        if parts.len() >= 8 {
                            let width: i32 = parts[1].parse().unwrap_or(200);
                            let height: i32 = parts[2].parse().unwrap_or(200);
                            let colspan: i32 = parts[3].parse().unwrap_or(1);
                            let rowspan: i32 = parts[4].parse().unwrap_or(1);
                            let col: i32 = parts[5].parse().unwrap_or(0);
                            let row: i32 = parts[6].parse().unwrap_or(0);
                            let path = parts[7].trim();

                            let button = Button::builder()
                                .tooltip_text(format!("Open folder: {}", path).as_str())
                                .build();
                            button.set_size_request(width, height);
                            if width != height {
                                let button_box = GtkBox::new(
                                    if width > height {
                                        Orientation::Horizontal
                                    } else {
                                        Orientation::Vertical
                                    }, 20);
                                
                                let image = gtk4::Image::from_icon_name("folder");
                                image.set_pixel_size(100);
                                button_box.append(&image);
                                button_box.append(&Label::new(Some(format!("{}",path).as_str())));
                                button.set_child(Some(&button_box));
                            } else {
                                let image = gtk4::Image::from_icon_name("folder");
                                image.set_pixel_size(100);
                                button.set_child(Some(&image));
                            }

                            let guesture = GestureClick::builder()
                                .button(0)
                                .build();

                            let path = path.to_string();
                            let mainbox_clone = mainbox_clone.clone();
                            guesture.connect_pressed(move |guesture, _, _, _| {
                                match guesture.current_button() {
                                    1 => {
                                        // left click
                                        if let Err(e) = std::process::Command::new("nautilus")
                                            .arg(&path)
                                            .spawn()
                                        {
                                            eprintln!("Failed to open folder: {}", e);
                                        }
                                    }
                                    3 => {
                                        // right click
                                        if let Err(e) = std::process::Command::new("xdg-open")
                                            .arg(&path)
                                            .spawn()
                                        {
                                            eprintln!("Failed to open folder: {}", e);
                                        }
                                    }
                                    _ => {
                                        eprintln!("click not registered");
                                    }
                                }
                                mainbox_clone.remove_css_class("scale-in");
                                mainbox_clone.add_css_class("scale-out");

                                let widget_clone = mainbox_clone.clone();
                                glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                                    widget_clone.set_visible(false);
                                    while let Some(child) = widget_clone.first_child() {
                                        widget_clone.remove(&child);
                                    }
                                });
                            });

                            button.add_controller(guesture);
                            grid_clone_inner2.attach(&button, col, row, colspan, rowspan);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to load config: {}", e);
            }
        }
        glib::ControlFlow::Break
    });
    


    let widgets_page = Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideDown)
        .transition_duration(500)
        .child(&grid)
        .reveal_child(true)
        .hexpand(true)
        .vexpand(true)
        .build();

    let hbox = GtkBox::new(Orientation::Horizontal, 6);
    let search = Entry::new();
    search.set_text("search");
    search.set_hexpand(true);
    search.set_halign(gtk4::Align::Center);

    hbox.append(&search);
    let menu = &Button::builder()
        .icon_name("edit-symbolic")
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::Baseline)
        .build();
    menu.set_css_classes(&["menu"]);
    hbox.append(menu);

    let search_page = Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideRight)
        .transition_duration(500)
        .child(&hbox)
        .reveal_child(true)
        .build();

    let control_bar = GtkBox::new(Orientation::Horizontal, 0);
    control_bar.append(&search_page);

    mainbox.append(&dummy);
    mainbox.append(&widgets_page);
    mainbox.append(&control_bar);
}