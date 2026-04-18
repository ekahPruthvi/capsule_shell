use std::collections::HashMap;
use std::fs;
use std::path::Path;

const DAT_PATH: &str = "/var/lib/cynager/desktop/widgets.dat";

pub fn load_positions() -> HashMap<String, (i32, i32)> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string(DAT_PATH) else { return map };
    for line in content.lines() {
        let Some((name, coords)) = line.split_once('=') else { continue };
        let Some((x, y)) = coords.split_once(',') else { continue };
        if let (Ok(x), Ok(y)) = (x.trim().parse(), y.trim().parse()) {
            map.insert(name.trim().to_string(), (x, y));
        }
    }
    map
}

pub fn save_position(name: &str, x: i32, y: i32) {
    let mut positions = load_positions();
    positions.insert(name.to_string(), (x, y));

    if let Some(parent) = Path::new(DAT_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let content = positions
        .iter()
        .map(|(k, (x, y))| format!("{k}={x},{y}"))
        .collect::<Vec<_>>()
        .join("\n");

    let _ = fs::write(DAT_PATH, content);
}