use gtk::prelude::*;
use gtk4 as gtk;

pub(crate) fn font_awesome_label(icon: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_markup(&format!(
        "<span font_family=\"Font Awesome 7 Free\" font_weight=\"900\" fallback=\"false\">{icon}</span>"
    ));
    label
}

pub(crate) fn icon_text_label(icon: &str, text: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.append(&font_awesome_label(icon));
    row.append(&gtk::Label::new(Some(text)));
    row
}

pub(crate) fn icon_button(icon: &str) -> gtk::Button {
    let button = gtk::Button::new();
    button.set_child(Some(&font_awesome_label(icon)));
    button
}

pub(crate) fn field_label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_xalign(0.0);
    label.set_valign(gtk::Align::Center);
    label.add_css_class("task-editor-field-label");
    label
}
