#[macro_use]
extern crate tr;

mod app;
mod config;
mod gui;
mod i18n;
mod jd;

use adw::prelude::*;
use gtk4::glib;

const APP_ID: &str = "io.github.xfuentes.gdownloader";

fn main() -> glib::ExitCode {
    env_logger::init();
    i18n::init();

    let app = adw::Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
        gui::build_ui(app);
    });

    app.run()
}
