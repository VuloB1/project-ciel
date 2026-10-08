// Prevents an additional console window from opening on Windows in release
// builds. Do not remove.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    aivt_companion_lib::run();
}
