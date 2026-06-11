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

pub(crate) fn settings_entry(label: &str, value: &str, content: &gtk::Box) -> gtk::Entry {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name = gtk::Label::new(Some(label));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_width_chars(18);
    row.append(&name);
    row.append(&entry);
    content.append(&row);
    entry
}

pub(crate) fn text_buffer_string(buffer: &gtk::TextBuffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}

pub(crate) fn update_entry_width(entry: &gtk::Entry) {
    const MIN_RENAME_ENTRY_WIDTH: i32 = 32;
    const MAX_RENAME_ENTRY_WIDTH: i32 = 560;
    const RENAME_ENTRY_HORIZONTAL_PADDING: i32 = 20;

    let text = entry.text();
    let layout = entry.create_pango_layout(Some(if text.is_empty() { " " } else { text.as_str() }));
    let (text_width, _) = layout.pixel_size();
    let width = (text_width + RENAME_ENTRY_HORIZONTAL_PADDING)
        .clamp(MIN_RENAME_ENTRY_WIDTH, MAX_RENAME_ENTRY_WIDTH);
    entry.set_width_chars(0);
    entry.set_max_width_chars(0);
    entry.set_size_request(width, -1);
}
