use std::rc::Rc;

use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::Keybindings;

pub(crate) struct KeybindingActions {
    pub(crate) create_task: Rc<dyn Fn()>,
    pub(crate) open_search: Rc<dyn Fn()>,
    pub(crate) close_overlay: Rc<dyn Fn()>,
    pub(crate) delete_task: Rc<dyn Fn()>,
    pub(crate) toggle_done: Rc<dyn Fn()>,
}

pub(crate) fn install_keybindings(
    window: &adw::ApplicationWindow,
    keybindings: &Keybindings,
    actions: KeybindingActions,
) {
    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let add_task = parse_accel(&keybindings.add_task);
    let search = parse_accel(&keybindings.search);
    let search_fallback = parse_accel("<Primary>f");
    let close_overlay = parse_accel(&keybindings.close_overlay);
    let delete_task = parse_accel(&keybindings.delete_task);
    let toggle_done = parse_accel(&keybindings.toggle_done);
    let window_for_keys = window.clone();
    key_controller.connect_key_pressed(move |_, key, _, modifiers| {
        let editing_text = window_has_text_focus(&window_for_keys);
        if editing_text
            && !modifiers.intersects(
                gtk::gdk::ModifierType::CONTROL_MASK
                    | gtk::gdk::ModifierType::META_MASK
                    | gtk::gdk::ModifierType::ALT_MASK,
            )
            && key != gtk::gdk::Key::Escape
        {
            return gtk::glib::Propagation::Proceed;
        }
        if accel_matches(add_task, key, modifiers) {
            (actions.create_task)();
            gtk::glib::Propagation::Stop
        } else if accel_matches(search, key, modifiers)
            || accel_matches(search_fallback, key, modifiers)
        {
            (actions.open_search)();
            gtk::glib::Propagation::Stop
        } else if key == gtk::gdk::Key::Escape || accel_matches(close_overlay, key, modifiers) {
            (actions.close_overlay)();
            gtk::glib::Propagation::Stop
        } else if accel_matches(delete_task, key, modifiers) {
            if editing_text {
                gtk::glib::Propagation::Proceed
            } else {
                (actions.delete_task)();
                gtk::glib::Propagation::Stop
            }
        } else if accel_matches(toggle_done, key, modifiers) {
            if editing_text {
                gtk::glib::Propagation::Proceed
            } else {
                (actions.toggle_done)();
                gtk::glib::Propagation::Stop
            }
        } else {
            gtk::glib::Propagation::Proceed
        }
    });
    window.add_controller(key_controller);
}

pub(crate) fn window_has_text_focus(window: &adw::ApplicationWindow) -> bool {
    gtk::prelude::GtkWindowExt::focus(window).is_some_and(|widget| {
        widget.is::<gtk::Entry>()
            || widget.is::<gtk::SearchEntry>()
            || widget.is::<gtk::TextView>()
            || widget.is::<gtk::EditableLabel>()
            || widget.is::<gtk::SpinButton>()
            || widget.ancestor(gtk::Entry::static_type()).is_some()
            || widget.ancestor(gtk::SearchEntry::static_type()).is_some()
            || widget.ancestor(gtk::TextView::static_type()).is_some()
            || widget.ancestor(gtk::EditableLabel::static_type()).is_some()
            || widget.ancestor(gtk::SpinButton::static_type()).is_some()
    })
}

fn parse_accel(accelerator: &str) -> Option<(gtk::gdk::Key, gtk::gdk::ModifierType)> {
    gtk::accelerator_parse(accelerator)
}

fn accel_matches(
    parsed: Option<(gtk::gdk::Key, gtk::gdk::ModifierType)>,
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) -> bool {
    let Some((expected_key, expected_modifiers)) = parsed else {
        return false;
    };
    key == expected_key && modifiers.contains(expected_modifiers)
}
