use gtk4::{
    glib, prelude::*, Box as GtkBox, Button, Entry, Grid, Label, Orientation, Revealer
};

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
        let mut count = 1;
        // let skip_indices = [6, 7, 13, 14];

        for row in 0..rows {
            for col in 0..cols {
                // if skip_indices.contains(&count) {
                //     count+= 1;
                // } else {
                    let button = Button::with_label(&format!("{}", count));
                    button.set_size_request(200, 200);
                    grid_clone.attach(&button, col as i32, row as i32, 1, 1);
                    count += 1;
                // }
            }
        }

        // let button = Button::with_label(&format!("{}", count));
        // button.set_size_request(400, 400);
        // grid_clone.attach(&button, 5, 0, 2, 2);
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
    hbox.append(&Button::builder()
        .icon_name("open-menu-symbolic")
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::Baseline)
        .build()
    );

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