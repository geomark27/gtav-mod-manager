//! Binary entry point of ModSwitch.

// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    gmm_desktop_lib::run()
}
