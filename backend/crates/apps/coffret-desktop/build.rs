//! Tauri's build step: reads `tauri.conf.json` (with `tauri.linux.conf.json`
//! merged over it on Linux, and `TAURI_CONFIG` over both), checks the icons and
//! the `ui/` directory exist, and generates the context `generate_context!`
//! embeds.

fn main() {
    tauri_build::build();
}
