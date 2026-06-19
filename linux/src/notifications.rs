use std::path::{Path, PathBuf};
use std::process::Command;

use taskmanager_core::{CoreResult, PlatformError, Task, TaskManagerCore};
use uuid::Uuid;

use crate::time::now_ms;

pub(crate) trait NotificationScheduler {
    fn schedule(&self, task_id: Uuid, fire_at: i64) -> CoreResult<()>;
    fn cancel(&self, task_id: Uuid) -> CoreResult<()>;
}

#[derive(Clone, Debug)]
pub(crate) struct SystemdUserNotificationScheduler {
    unit_dir: PathBuf,
    executable: PathBuf,
    database_path: Option<PathBuf>,
}

impl SystemdUserNotificationScheduler {
    pub(crate) fn new(database_path: Option<PathBuf>) -> CoreResult<Self> {
        Ok(Self {
            unit_dir: default_systemd_user_unit_dir()?,
            executable: std::env::current_exe().map_err(|error| {
                PlatformError::OperationFailed(format!(
                    "failed to resolve notification helper executable: {error}"
                ))
            })?,
            database_path,
        })
    }

    #[cfg(test)]
    fn with_paths(unit_dir: PathBuf, executable: PathBuf, database_path: Option<PathBuf>) -> Self {
        Self {
            unit_dir,
            executable,
            database_path,
        }
    }

    fn service_path(&self, task_id: Uuid) -> PathBuf {
        self.unit_dir
            .join(format!("{}.service", unit_basename(task_id)))
    }

    fn timer_path(&self, task_id: Uuid) -> PathBuf {
        self.unit_dir
            .join(format!("{}.timer", unit_basename(task_id)))
    }

    fn write_units(&self, task_id: Uuid, service: &str, timer: &str) -> CoreResult<()> {
        std::fs::create_dir_all(&self.unit_dir).map_err(|error| {
            PlatformError::OperationFailed(format!(
                "failed to create systemd user unit dir {}: {error}",
                self.unit_dir.display()
            ))
        })?;
        let service_name = format!("{}.service", unit_basename(task_id));
        let timer_name = format!("{}.timer", unit_basename(task_id));
        std::fs::write(self.service_path(task_id), service).map_err(|error| {
            PlatformError::OperationFailed(format!("failed to write {service_name}: {error}"))
        })?;
        std::fs::write(self.timer_path(task_id), timer).map_err(|error| {
            PlatformError::OperationFailed(format!("failed to write {timer_name}: {error}"))
        })?;
        Ok(())
    }

    fn units_match(&self, task_id: Uuid, service: &str, timer: &str) -> bool {
        std::fs::read_to_string(self.service_path(task_id))
            .is_ok_and(|existing| existing == service)
            && std::fs::read_to_string(self.timer_path(task_id))
                .is_ok_and(|existing| existing == timer)
    }

    fn remove_units(&self, task_id: Uuid) -> CoreResult<()> {
        remove_file_if_exists(self.timer_path(task_id))?;
        remove_file_if_exists(self.service_path(task_id))?;
        Ok(())
    }
}

impl NotificationScheduler for SystemdUserNotificationScheduler {
    fn schedule(&self, task_id: Uuid, fire_at: i64) -> CoreResult<()> {
        let service_name = format!("{}.service", unit_basename(task_id));
        let timer_name = format!("{}.timer", unit_basename(task_id));
        let service = service_unit(&self.executable, self.database_path.as_deref(), task_id);
        let timer = timer_unit(&service_name, fire_at)?;
        if self.units_match(task_id, &service, &timer) {
            systemctl_user(&["enable", "--now", &timer_name])?;
            return Ok(());
        }

        let calendar_time = systemd_on_calendar_utc(fire_at)?;
        self.remove_units(task_id)?;
        self.write_units(task_id, &service, &timer)?;
        systemctl_user(&["daemon-reload"])?;
        systemctl_user(&["enable", &timer_name])?;
        systemctl_user(&["restart", &timer_name])?;
        eprintln!("Scheduled reminder for task {task_id} at {calendar_time}");
        Ok(())
    }

    fn cancel(&self, task_id: Uuid) -> CoreResult<()> {
        let timer_exists = self.timer_path(task_id).exists();
        let service_exists = self.service_path(task_id).exists();
        if !timer_exists && !service_exists {
            return Ok(());
        }

        let timer_name = format!("{}.timer", unit_basename(task_id));
        let _ = systemctl_user(&["disable", "--now", &timer_name]);
        self.remove_units(task_id)?;
        let _ = systemctl_user(&["daemon-reload"]);
        eprintln!("Canceled reminder for task {task_id}");
        Ok(())
    }
}

pub(crate) fn reconcile_task_notification(
    scheduler: &dyn NotificationScheduler,
    task: &Task,
    now_ms: i64,
) -> CoreResult<()> {
    if let Some(fire_at) = task.schedulable_notification_at(now_ms) {
        scheduler.schedule(task.id, fire_at)
    } else {
        scheduler.cancel(task.id)
    }
}

pub(crate) fn emit_task_reminder(database_path: &Path, task_id: Uuid) -> CoreResult<()> {
    let core = TaskManagerCore::open(database_path)?;
    let task = core.get_task(task_id)?;
    if task.notification_due(now_ms())
        && core
            .vault_settings()
            .map(|settings| settings.notification_sound != "silent")
            .unwrap_or(true)
    {
        show_desktop_notification(task.id, &task.title)?;
    }
    Ok(())
}

pub(crate) fn show_desktop_notification(task_id: Uuid, title: &str) -> CoreResult<()> {
    notify_rust::Notification::new()
        .appname("tsk")
        .summary("tsk reminder")
        .body(title)
        .id(notification_id(task_id))
        .show()
        .map(|_| ())
        .map_err(|error| {
            PlatformError::OperationFailed(format!("failed to show notification: {error}")).into()
        })
}

pub(crate) fn unit_basename(task_id: Uuid) -> String {
    format!("tsk-reminder-{}", task_id.as_simple())
}

fn default_systemd_user_unit_dir() -> CoreResult<PathBuf> {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(config_home).join("systemd/user"));
    }
    let home = std::env::var_os("HOME").ok_or_else(|| {
        PlatformError::OperationFailed(
            "HOME is not set; cannot resolve systemd user dir".to_owned(),
        )
    })?;
    Ok(PathBuf::from(home).join(".config/systemd/user"))
}

fn remove_file_if_exists(path: PathBuf) -> CoreResult<()> {
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(PlatformError::OperationFailed(format!(
            "failed to remove {}: {error}",
            path.display()
        ))
        .into()),
    }
}

fn systemctl_user(args: &[&str]) -> CoreResult<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| {
            PlatformError::OperationFailed(format!("failed to run systemctl --user: {error}"))
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(PlatformError::OperationFailed(format!(
            "systemctl --user {} exited with {status}",
            args.join(" ")
        ))
        .into())
    }
}

fn service_unit(executable: &Path, database_path: Option<&Path>, task_id: Uuid) -> String {
    let db_arg = database_path
        .map(|path| format!(" --db {}", systemd_quote(path)))
        .unwrap_or_default();
    format!(
        "[Unit]\nDescription=tsk reminder for {task_id}\n\n[Service]\nType=oneshot\nExecStart={} --emit-reminder {task_id}{db_arg}\n",
        systemd_quote(executable)
    )
}

fn timer_unit(service_name: &str, fire_at: i64) -> CoreResult<String> {
    Ok(format!(
        "[Unit]\nDescription=tsk reminder timer\n\n[Timer]\nOnCalendar={}\nAccuracySec=1s\nPersistent=true\nUnit={service_name}\n\n[Install]\nWantedBy=timers.target\n",
        systemd_on_calendar_utc(fire_at)?
    ))
}

pub(crate) fn systemd_on_calendar_utc(fire_at_ms: i64) -> CoreResult<String> {
    let unix_seconds = ceil_millis_to_seconds(fire_at_ms);
    let time = time::OffsetDateTime::from_unix_timestamp(unix_seconds).map_err(|error| {
        PlatformError::OperationFailed(format!("invalid reminder time {fire_at_ms}: {error}"))
    })?;
    Ok(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        time.year(),
        u8::from(time.month()),
        time.day(),
        time.hour(),
        time.minute(),
        time.second()
    ))
}

fn ceil_millis_to_seconds(value: i64) -> i64 {
    value.div_euclid(1000) + i64::from(value.rem_euclid(1000) != 0)
}

fn systemd_quote(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn notification_id(task_id: Uuid) -> u32 {
    let bytes = task_id.as_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use taskmanager_core::TaskStatus;

    #[test]
    fn unit_basename_is_deterministic_and_safe() {
        let task_id = Uuid::parse_str("018f6f4a-c9f4-7724-91ef-2f7b38a62600").unwrap();
        assert_eq!(
            unit_basename(task_id),
            "tsk-reminder-018f6f4ac9f4772491ef2f7b38a62600"
        );
    }

    #[test]
    fn calendar_format_uses_utc_and_ceil_seconds() {
        assert_eq!(
            systemd_on_calendar_utc(1_717_603_200_001).unwrap(),
            "2024-06-05 16:00:01 UTC"
        );
    }

    #[test]
    fn service_unit_stores_only_task_id_and_database_path_not_title() {
        let task_id = Uuid::parse_str("018f6f4a-c9f4-7724-91ef-2f7b38a62600").unwrap();
        let unit = service_unit(
            Path::new("/tmp/tsk gui"),
            Some(Path::new("/tmp/tasks test.sqlite3")),
            task_id,
        );
        assert!(unit.contains("--emit-reminder 018f6f4a-c9f4-7724-91ef-2f7b38a62600"));
        assert!(unit.contains("--db \"/tmp/tasks test.sqlite3\""));
        assert!(!unit.contains("secret title"));
        assert!(unit.contains("ExecStart=\"/tmp/tsk gui\""));
    }

    #[test]
    fn reconcile_uses_core_notification_semantics() {
        #[derive(Default)]
        struct FakeScheduler {
            scheduled: std::cell::RefCell<Vec<(Uuid, i64)>>,
            canceled: std::cell::RefCell<Vec<Uuid>>,
        }
        impl NotificationScheduler for FakeScheduler {
            fn schedule(&self, task_id: Uuid, fire_at: i64) -> CoreResult<()> {
                self.scheduled.borrow_mut().push((task_id, fire_at));
                Ok(())
            }
            fn cancel(&self, task_id: Uuid) -> CoreResult<()> {
                self.canceled.borrow_mut().push(task_id);
                Ok(())
            }
        }

        let task = Task {
            id: Uuid::new_v4(),
            title: "title".to_owned(),
            body: String::new(),
            due_at: Some(2_000),
            reminder_offset_ms: Some(500),
            status: TaskStatus::Open,
            project_id: None,
            tags: Vec::new(),
            created_at: 0,
            updated_at: 0,
            deleted: false,
            dirty: false,
        };
        let scheduler = FakeScheduler::default();
        reconcile_task_notification(&scheduler, &task, 1_000).unwrap();
        assert_eq!(*scheduler.scheduled.borrow(), vec![(task.id, 1_500)]);

        let mut done = task.clone();
        done.status = TaskStatus::Done;
        reconcile_task_notification(&scheduler, &done, 1_000).unwrap();
        assert_eq!(*scheduler.canceled.borrow(), vec![task.id]);
    }

    #[test]
    fn systemd_scheduler_writes_units_without_systemctl() {
        let temp = tempfile::tempdir().unwrap();
        let task_id = Uuid::new_v4();
        let scheduler = SystemdUserNotificationScheduler::with_paths(
            temp.path().to_path_buf(),
            PathBuf::from("/usr/bin/tsk-gui"),
            Some(PathBuf::from("/tmp/tasks.sqlite3")),
        );
        let service = service_unit(
            &scheduler.executable,
            scheduler.database_path.as_deref(),
            task_id,
        );
        let timer = timer_unit(
            &format!("{}.service", unit_basename(task_id)),
            1_717_603_200_000,
        )
        .unwrap();
        scheduler.write_units(task_id, &service, &timer).unwrap();
        let service = std::fs::read_to_string(scheduler.service_path(task_id)).unwrap();
        let timer = std::fs::read_to_string(scheduler.timer_path(task_id)).unwrap();
        assert!(service.contains("--emit-reminder"));
        assert!(timer.contains("OnCalendar=2024-06-05 16:00:00 UTC"));
    }
}
