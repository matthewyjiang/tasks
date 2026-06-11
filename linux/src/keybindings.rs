use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

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

pub(crate) fn parse_accel(accelerator: &str) -> Option<(gtk::gdk::Key, gtk::gdk::ModifierType)> {
    gtk::accelerator_parse(accelerator)
}

pub(crate) fn accel_matches(
    parsed: Option<(gtk::gdk::Key, gtk::gdk::ModifierType)>,
    key: gtk::gdk::Key,
    modifiers: gtk::gdk::ModifierType,
) -> bool {
    let Some((expected_key, expected_modifiers)) = parsed else {
        return false;
    };
    key == expected_key && modifiers.contains(expected_modifiers)
}
