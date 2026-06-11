use gtk::prelude::*;
use gtk4 as gtk;
use taskmanager_core::TaskList;

use crate::ui::widgets::font_awesome_label;

pub(crate) fn user_list_row(list: &TaskList) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("sidebar-row");
    row.set_widget_name(&list.id.to_string());
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);
    row_box.set_margin_start(10);
    row_box.set_margin_end(10);
    let icon = font_awesome_label("\u{f03a}");
    icon.add_css_class("sidebar-icon");
    icon.add_css_class("sidebar-icon-list");
    let name = gtk::Label::new(Some(&list.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row_box.append(&icon);
    row_box.append(&name);
    row.set_child(Some(&row_box));
    row
}
