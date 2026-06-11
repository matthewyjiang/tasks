use gtk::prelude::*;
use gtk4 as gtk;
use regex::Regex;
use taskmanager_core::Task;

use crate::task_format::format_task_row_summary;

pub(crate) struct MoveListPanelWidgets {
    pub(crate) panel: gtk::Box,
    pub(crate) search_entry: gtk::SearchEntry,
    pub(crate) results: gtk::ListBox,
}

pub(crate) fn build_move_list_panel() -> MoveListPanelWidgets {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 14);
    panel.add_css_class("move-list-panel");
    panel.set_width_request(460);
    panel.set_halign(gtk::Align::Center);
    panel.set_valign(gtk::Align::Center);
    panel.set_opacity(0.0);
    panel.set_visible(false);
    let search_entry = gtk::SearchEntry::new();
    search_entry.add_css_class("move-list-search");
    search_entry.set_placeholder_text(Some("Search lists with regex"));
    let results = gtk::ListBox::new();
    results.add_css_class("move-list-results");
    results.set_selection_mode(gtk::SelectionMode::None);
    let scroll = gtk::ScrolledWindow::builder()
        .min_content_height(260)
        .child(&results)
        .build();
    panel.append(&search_entry);
    panel.append(&scroll);

    MoveListPanelWidgets {
        panel,
        search_entry,
        results,
    }
}

pub(crate) fn normalize_query(query: &str) -> String {
    query.trim().to_owned()
}

pub(crate) fn task_search_result_row(task: &Task) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(&task.id.to_string());
    row.add_css_class("search-result-row");
    let content = gtk::Box::new(gtk::Orientation::Vertical, 2);
    content.set_margin_top(8);
    content.set_margin_bottom(8);
    content.set_margin_start(10);
    content.set_margin_end(10);
    let title = gtk::Label::new(Some(&task.title));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let summary = gtk::Label::new(Some(&format_task_row_summary(task)));
    summary.set_xalign(0.0);
    summary.set_ellipsize(gtk::pango::EllipsizeMode::End);
    summary.add_css_class("task-summary");
    content.append(&title);
    content.append(&summary);
    row.set_child(Some(&content));
    row
}

pub(crate) fn move_list_result_row(id: &str, name: &str) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.set_widget_name(id);
    row.add_css_class("search-result-row");
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_margin_top(10);
    label.set_margin_bottom(10);
    label.set_margin_start(12);
    label.set_margin_end(12);
    row.set_child(Some(&label));
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_query_trims_whitespace() {
        assert_eq!(normalize_query("  open  "), "open");
    }
}

pub(crate) fn regex_from_query(query: &str) -> Result<Regex, regex::Error> {
    let pattern = if query.is_empty() { ".*" } else { query };
    Regex::new(&format!("(?i){pattern}"))
}

pub(crate) fn regex_matches_task(regex: &Regex, task: &Task) -> bool {
    regex.is_match(&task.title)
        || regex.is_match(&task.body)
        || task.tags.iter().any(|tag| regex.is_match(tag))
}
