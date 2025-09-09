// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

extern crate blas_src;

fn main() {
    autoeq_ui_lib::run()
}
