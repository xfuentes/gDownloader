use gtk4::prelude::*;
use gtk4::glib;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct WaitDialog {
    window: gtk4::Window,
    pulse: Rc<RefCell<Option<glib::SourceId>>>,
}

impl WaitDialog {
    pub fn show(parent: &impl IsA<gtk4::Window>, message: &str) -> Self {
        let window = gtk4::Window::builder()
            .transient_for(parent)
            .modal(true)
            .deletable(false)
            .resizable(false)
            .build();
        window.set_titlebar(None::<&gtk4::HeaderBar>);

        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        vbox.set_margin_start(24);
        vbox.set_margin_end(24);
        vbox.set_margin_top(24);
        vbox.set_margin_bottom(24);

        let label = gtk4::Label::new(Some(message));
        let progress = gtk4::ProgressBar::new();
        progress.set_pulse_step(0.1);

        vbox.append(&label);
        vbox.append(&progress);
        window.set_child(Some(&vbox));
        window.present();

        let progress_c = progress.clone();
        let pulse = glib::source::timeout_add_local(
            Duration::from_millis(100),
            move || {
                progress_c.pulse();
                glib::ControlFlow::Continue
            },
        );

        Self {
            window,
            pulse: Rc::new(RefCell::new(Some(pulse))),
        }
    }

    pub fn close(&self) {
        if let Some(p) = self.pulse.borrow_mut().take() {
            p.remove();
        }
        self.window.close();
    }
}
