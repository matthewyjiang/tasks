use gtk::prelude::*;
use gtk4 as gtk;
use taskmanager_core::{Task, TaskList, TaskStatus};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ListProgress {
    pub(crate) completed: usize,
    pub(crate) total: usize,
}

impl ListProgress {
    pub(crate) fn ratio(self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.completed as f64 / self.total as f64
        }
    }
}

pub(crate) fn list_progress(list: &TaskList, tasks: &[Task]) -> ListProgress {
    let mut completed = 0;
    let mut total = 0;
    for task in tasks.iter().filter(|task| task.project_id == Some(list.id)) {
        total += 1;
        if task.status == TaskStatus::Done {
            completed += 1;
        }
    }
    ListProgress { completed, total }
}

pub(crate) fn user_list_row(list: &TaskList, progress: ListProgress) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::new();
    row.add_css_class("sidebar-row");
    row.set_widget_name(&list.id.to_string());
    let row_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row_box.set_margin_top(8);
    row_box.set_margin_bottom(8);
    row_box.set_margin_start(10);
    row_box.set_margin_end(10);
    let progress_indicator = list_progress_indicator(progress);
    let name = gtk::Label::new(Some(&list.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row_box.append(&progress_indicator);
    row_box.append(&name);
    row.set_child(Some(&row_box));
    row
}

fn list_progress_indicator(progress: ListProgress) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.add_css_class("list-progress");
    area.set_content_width(18);
    area.set_content_height(18);
    area.set_width_request(18);
    area.set_height_request(18);
    area.set_tooltip_text(Some(&format!(
        "{} of {} tasks complete",
        progress.completed, progress.total
    )));
    area.set_draw_func(move |_, cr, width, height| {
        let size = f64::from(width.min(height));
        let center = size / 2.0;
        let radius = (size / 2.0 - 2.0).max(1.0);
        let line_width = 2.0;
        let ratio = progress.ratio().clamp(0.0, 1.0);

        cr.set_line_width(line_width);
        cr.set_line_cap(gtk::cairo::LineCap::Round);

        // Neutral track, including the empty-list state.
        cr.set_source_rgba(
            0.55,
            0.55,
            0.58,
            if progress.total == 0 { 0.28 } else { 0.34 },
        );
        cr.arc(center, center, radius, 0.0, std::f64::consts::TAU);
        let _ = cr.stroke();

        if progress.total == 0 || ratio == 0.0 {
            return;
        }

        // Things-like blue partial ring, starting from 12 o'clock.
        let start = -std::f64::consts::FRAC_PI_2;
        let end = start + (std::f64::consts::TAU * ratio);
        cr.set_source_rgba(0.0, 0.48, 1.0, 0.95);
        cr.arc(center, center, radius, start, end);
        let _ = cr.stroke();

        if ratio >= 1.0 {
            cr.set_source_rgba(0.0, 0.48, 1.0, 0.12);
            cr.arc(
                center,
                center,
                radius - line_width,
                0.0,
                std::f64::consts::TAU,
            );
            let _ = cr.fill();
        }
    });
    area
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn task(project_id: Option<Uuid>, status: TaskStatus) -> Task {
        Task {
            id: Uuid::new_v4(),
            title: "task".to_owned(),
            body: String::new(),
            due_at: None,
            status,
            project_id,
            tags: Vec::new(),
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        }
    }

    fn list(id: Uuid) -> TaskList {
        TaskList {
            id,
            name: "List".to_owned(),
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        }
    }

    #[test]
    fn progress_counts_completed_over_total_for_matching_list() {
        let list_id = Uuid::new_v4();
        let other_id = Uuid::new_v4();
        let progress = list_progress(
            &list(list_id),
            &[
                task(Some(list_id), TaskStatus::Done),
                task(Some(list_id), TaskStatus::Open),
                task(Some(other_id), TaskStatus::Done),
                task(None, TaskStatus::Done),
            ],
        );

        assert_eq!(progress.completed, 1);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.ratio(), 0.5);
    }

    #[test]
    fn empty_list_progress_is_zero_without_dividing_by_zero() {
        let progress = list_progress(&list(Uuid::new_v4()), &[]);

        assert_eq!(progress.completed, 0);
        assert_eq!(progress.total, 0);
        assert_eq!(progress.ratio(), 0.0);
    }
}
