use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use taskmanager_core::TaskManagerCore;

use super::AppState;
use crate::sync::{linux_sync_configured, run_linux_sync, LinuxSyncSummary};
use crate::time::now_ms;
use crate::ui::settings::{read_settings, write_settings, SyncStatus};

impl AppState {
    pub(super) fn request_sync(self: &Rc<Self>) {
        self.request_sync_with_feedback(false);
    }

    pub(super) fn request_manual_sync(self: &Rc<Self>) {
        self.request_sync_with_feedback(true);
    }

    fn request_sync_with_feedback(self: &Rc<Self>, notify: bool) {
        if *self.sync_in_progress.borrow() {
            if !notify {
                self.sync_pending.replace(true);
            }
            return;
        }
        if !linux_sync_configured(&self.settings_path) {
            if notify {
                self.toast("Sync is not configured yet.".to_owned());
            }
            return;
        }
        self.set_sync_running(true);
        self.sync_in_progress.replace(true);
        let db_path = self.db_path.clone();
        let settings_path = self.settings_path.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = run_linux_sync(&db_path, &settings_path);
            let _ = sender.send(result);
        });
        let state = Rc::clone(self);
        gtk::glib::timeout_add_local(
            std::time::Duration::from_millis(120),
            move || match receiver.try_recv() {
                Ok(result) => {
                    state.sync_in_progress.replace(false);
                    state.set_sync_running(false);
                    record_sync_status(&state.settings_path, &state.core, &result);
                    match result {
                        Ok(summary) => {
                            if summary.changed() {
                                state.load_tasks();
                                state.reconcile_notifications();
                            }
                            if notify {
                                state.toast(format_sync_status(&summary));
                            }
                        }
                        Err(error) => state.toast(format!("Sync failed: {error}")),
                    }
                    if state.sync_pending.replace(false) {
                        state.request_sync();
                    }
                    gtk::glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => gtk::glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    state.sync_in_progress.replace(false);
                    state.set_sync_running(false);
                    state.toast("Sync failed: worker disconnected".to_owned());
                    gtk::glib::ControlFlow::Break
                }
            },
        );
    }

    fn set_sync_running(&self, running: bool) {
        self.sync_button.set_sensitive(!running);
        self.sync_icon.set_visible(!running);
        self.sync_activity.set_visible(running);
        if running {
            self.sync_stack.set_visible_child_name("activity");
            self.sync_button.set_tooltip_text(Some("Syncing…"));
        } else {
            self.sync_stack.set_visible_child_name("icon");
            self.sync_button.set_tooltip_text(Some("Sync now"));
        }
    }
}

fn record_sync_status(
    settings_path: &std::path::Path,
    core: &TaskManagerCore,
    result: &taskmanager_core::CoreResult<LinuxSyncSummary>,
) {
    let mut settings = read_settings(settings_path).unwrap_or_default();
    let now = now_ms();
    let core_status = core.sync_status().ok();
    let pending_retries = core_status
        .as_ref()
        .map(|status| status.retry_queue_depth)
        .unwrap_or(settings.sync_status.pending_retries);
    let dirty_count = core_status
        .as_ref()
        .map(|status| status.dirty_count)
        .unwrap_or(settings.sync_status.dirty_count);
    let cursor = core_status
        .map(|status| status.cursor)
        .unwrap_or(settings.sync_status.cursor);
    settings.sync_status = match result {
        Ok(summary) => SyncStatus {
            last_attempt_at: Some(now),
            last_success_at: Some(now),
            last_pushed: summary.pushed,
            last_pulled: summary.pulled,
            last_failed: summary.failed,
            last_error: String::new(),
            pending_retries,
            dirty_count,
            cursor,
            conflicts: summary.conflicts,
        },
        Err(error) => SyncStatus {
            last_attempt_at: Some(now),
            last_success_at: settings.sync_status.last_success_at,
            last_pushed: settings.sync_status.last_pushed,
            last_pulled: settings.sync_status.last_pulled,
            last_failed: settings.sync_status.last_failed,
            last_error: error.to_string(),
            pending_retries,
            dirty_count,
            cursor,
            conflicts: settings.sync_status.conflicts,
        },
    };
    if let Err(error) = write_settings(settings_path, &settings) {
        eprintln!("Failed to persist sync status: {error}");
    }
}

fn format_sync_status(summary: &LinuxSyncSummary) -> String {
    if summary.pushed == 0
        && summary.pulled == 0
        && summary.failed == 0
        && summary.pending_retries == 0
        && summary.conflicts == 0
    {
        "Synced. Everything is up to date.".to_owned()
    } else if summary.failed == 0 && summary.pending_retries == 0 && summary.conflicts == 0 {
        format!(
            "Synced. {} pushed, {} pulled.",
            summary.pushed, summary.pulled
        )
    } else {
        format!(
            "Synced with issues. {} pushed, {} pulled, {} failed, {} pending retry, {} conflict resolved automatically (last write wins).",
            summary.pushed, summary.pulled, summary.failed, summary.pending_retries, summary.conflicts
        )
    }
}

pub(super) fn build_sync_activity_icon(angle: Rc<Cell<f64>>) -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_content_width(18);
    area.set_content_height(18);
    area.set_draw_func(move |_, cr, width, height| {
        let size = f64::from(width.min(height));
        let center = size / 2.0;
        let radius = size * 0.31;
        cr.translate(f64::from(width) / 2.0, f64::from(height) / 2.0);
        cr.rotate(angle.get());
        cr.set_source_rgba(0.50, 0.58, 0.68, 1.0);
        cr.set_line_width(1.8);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.arc(0.0, 0.0, radius, -0.35, std::f64::consts::TAU - 0.95);
        let _ = cr.stroke();

        let tip_angle: f64 = -0.35;
        let tip_x = radius * tip_angle.cos();
        let tip_y = radius * tip_angle.sin();
        cr.move_to(tip_x, tip_y);
        cr.line_to(tip_x - center * 0.18, tip_y - center * 0.02);
        cr.move_to(tip_x, tip_y);
        cr.line_to(tip_x - center * 0.06, tip_y + center * 0.17);
        let _ = cr.stroke();
    });
    area
}
