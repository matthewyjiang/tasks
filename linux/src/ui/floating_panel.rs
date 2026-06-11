use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

const FLOATING_PANEL_FADE_MS: u64 = 180;
const TASK_EDITOR_MIN_WIDTH: i32 = 640;
const TASK_EDITOR_MAX_WIDTH: i32 = 1040;
const TASK_EDITOR_WIDTH_RATIO: f64 = 0.72;
const TASK_EDITOR_MIN_HEIGHT: i32 = 420;
const TASK_EDITOR_MAX_HEIGHT: i32 = 820;
const TASK_EDITOR_HEIGHT_RATIO: f64 = 0.78;
const SETTINGS_PANEL_MIN_WIDTH: i32 = 560;
const SETTINGS_PANEL_MAX_WIDTH: i32 = 900;
const SETTINGS_PANEL_WIDTH_RATIO: f64 = 0.62;
const SETTINGS_PANEL_MIN_HEIGHT: i32 = 420;
const SETTINGS_PANEL_MAX_HEIGHT: i32 = 760;
const SETTINGS_PANEL_HEIGHT_RATIO: f64 = 0.72;

pub(crate) fn resize_settings_panel(window: &adw::ApplicationWindow, settings_panel: &gtk::Box) {
    let width = window.allocated_width();
    let height = window.allocated_height();
    if width <= 0 || height <= 0 {
        return;
    }
    let panel_width = ((width as f64) * SETTINGS_PANEL_WIDTH_RATIO) as i32;
    let panel_height = ((height as f64) * SETTINGS_PANEL_HEIGHT_RATIO) as i32;
    settings_panel.set_size_request(
        panel_width.clamp(SETTINGS_PANEL_MIN_WIDTH, SETTINGS_PANEL_MAX_WIDTH),
        panel_height.clamp(SETTINGS_PANEL_MIN_HEIGHT, SETTINGS_PANEL_MAX_HEIGHT),
    );
}

pub(crate) fn resize_task_editor_panel(window: &adw::ApplicationWindow, editor_panel: &gtk::Box) {
    let width = window.allocated_width();
    let height = window.allocated_height();
    if width <= 0 || height <= 0 {
        return;
    }
    let editor_width = ((width as f64) * TASK_EDITOR_WIDTH_RATIO) as i32;
    let editor_height = ((height as f64) * TASK_EDITOR_HEIGHT_RATIO) as i32;
    editor_panel.set_size_request(
        editor_width.clamp(TASK_EDITOR_MIN_WIDTH, TASK_EDITOR_MAX_WIDTH),
        editor_height.clamp(TASK_EDITOR_MIN_HEIGHT, TASK_EDITOR_MAX_HEIGHT),
    );
}

pub(crate) fn show_floating_panel<W>(widget: &W)
where
    W: IsA<gtk::Widget> + Clone + 'static,
{
    widget.set_opacity(0.0);
    widget.set_visible(true);
    animate_opacity(widget, 0.0, 1.0, FLOATING_PANEL_FADE_MS, None);
}

pub(crate) fn hide_floating_panel<W>(widget: &W)
where
    W: IsA<gtk::Widget> + Clone + 'static,
{
    let start = widget.opacity();
    let widget_to_hide = widget.clone();
    animate_opacity(
        widget,
        start,
        0.0,
        FLOATING_PANEL_FADE_MS,
        Some(Box::new(move || widget_to_hide.set_visible(false))),
    );
}

pub(crate) fn animate_opacity<W>(
    widget: &W,
    from: f64,
    to: f64,
    duration_ms: u64,
    done: Option<Box<dyn FnOnce() + 'static>>,
) where
    W: IsA<gtk::Widget> + Clone + 'static,
{
    let widget = widget.clone();
    let started = std::time::Instant::now();
    let done = Rc::new(RefCell::new(done));
    gtk::glib::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let progress = (started.elapsed().as_millis() as f64 / duration_ms as f64).min(1.0);
        let eased = 1.0 - (1.0 - progress).powi(3);
        widget.set_opacity(from + (to - from) * eased);
        if progress >= 1.0 {
            if let Some(done) = done.borrow_mut().take() {
                done();
            }
            gtk::glib::ControlFlow::Break
        } else {
            gtk::glib::ControlFlow::Continue
        }
    });
}
