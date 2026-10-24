use crate::domain::{repo::RepoStatusDto, settings::RefreshSettings};
use serde::Serialize;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub const STATUS_EVENT: &str = "repo-status-snapshot";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusSnapshot {
    pub generation: u64,
    pub completed_at: u64,
    pub repos: Vec<RepoStatusDto>,
    pub error: Option<String>,
}

struct RefreshState {
    requested: u64,
    completed: u64,
    running: Option<u64>,
    snapshot: Option<StatusSnapshot>,
    interval: Option<Duration>,
    next_due: Option<Instant>,
}

impl RefreshState {
    fn new() -> Self {
        Self {
            requested: 1,
            completed: 0,
            running: None,
            snapshot: None,
            interval: None,
            next_due: None,
        }
    }

    fn request(&mut self, fresh: bool) -> u64 {
        if fresh && (self.requested == self.completed || self.running == Some(self.requested)) {
            self.requested += 1;
        }
        self.requested
    }

    fn configure(&mut self, settings: &RefreshSettings, now: Instant) {
        self.interval = settings
            .lightweight_refresh_enabled
            .then(|| Duration::from_secs(u64::from(settings.interval_minutes.clamp(1, 60)) * 60));
        self.next_due = self.interval.map(|interval| now + interval);
    }

    fn start(&mut self, now: Instant) -> Option<u64> {
        if self.running.is_some() {
            return None;
        }
        if self.requested == self.completed && self.next_due.is_some_and(|due| now >= due) {
            self.requested += 1;
        }
        if self.requested == self.completed {
            return None;
        }
        self.running = Some(self.requested);
        self.running
    }

    fn complete(&mut self, snapshot: StatusSnapshot, now: Instant) {
        self.completed = snapshot.generation;
        self.snapshot = Some(snapshot);
        self.running = None;
        self.next_due = self.interval.map(|interval| now + interval);
    }
}

pub struct StatusRefresh {
    state: Mutex<RefreshState>,
    changed: Condvar,
}

impl StatusRefresh {
    fn new() -> Self {
        Self {
            state: Mutex::new(RefreshState::new()),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RefreshState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn request(&self, fresh: bool) -> u64 {
        let ticket = self.lock().request(fresh);
        self.changed.notify_all();
        ticket
    }

    pub fn wait(&self, ticket: u64) -> StatusSnapshot {
        let mut state = self.lock();
        while state.completed < ticket {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
        state
            .snapshot
            .clone()
            .expect("completed refresh has a snapshot")
    }

    fn configure(&self, settings: &RefreshSettings) {
        let mut state = self.lock();
        state.configure(settings, Instant::now());
        state.request(true);
        self.changed.notify_all();
    }

    fn run(&self, app: AppHandle) {
        loop {
            let ticket = {
                let mut state = self.lock();
                loop {
                    if let Some(ticket) = state.start(Instant::now()) {
                        break ticket;
                    }
                    state = if let Some(due) = state.next_due {
                        self.changed
                            .wait_timeout(state, due.saturating_duration_since(Instant::now()))
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .0
                    } else {
                        self.changed
                            .wait(state)
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                    };
                }
            };
            let tray_generation = crate::tray_status::begin_tray_menu_update();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                crate::app_settings::load_app_settings(&app)
                    .and_then(|settings| crate::repo_status::collect_repo_statuses(settings.repos))
            }))
            .unwrap_or_else(|_| Err("仓库状态收集意外中断".into()));
            let (repos, error) = match result {
                Ok(repos) => (repos, None),
                Err(error) => (
                    self.lock()
                        .snapshot
                        .as_ref()
                        .map(|s| s.repos.clone())
                        .unwrap_or_default(),
                    Some(error),
                ),
            };
            let snapshot = StatusSnapshot {
                generation: ticket,
                completed_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                repos,
                error,
            };
            publish(&app, &snapshot, tray_generation);
            self.lock().complete(snapshot, Instant::now());
            self.changed.notify_all();
        }
    }
}

fn publish(app: &AppHandle, snapshot: &StatusSnapshot, tray_generation: u64) {
    let menu_result = match &snapshot.error {
        Some(error) => crate::tray_status::set_error_menu_if_current(app, tray_generation, error),
        None => {
            crate::tray_status::set_status_menu_if_current(app, tray_generation, &snapshot.repos)
        }
    };
    if let Err(error) = menu_result {
        crate::diagnostics::log("status_refresh.tray_error", error);
    }
    if snapshot.error.is_none() {
        if let Err(error) = crate::widget_data::write_widget_data(&snapshot.repos, tray_generation)
        {
            crate::diagnostics::log("widget_data.write_error", error);
        }
    }
    if let Err(error) = app.emit(STATUS_EVENT, snapshot) {
        crate::diagnostics::log("status_refresh.emit_error", error.to_string());
    }
}

pub fn service(app: &AppHandle) -> Arc<StatusRefresh> {
    app.state::<Arc<StatusRefresh>>().inner().clone()
}

pub fn settings_changed(app: &AppHandle, settings: &RefreshSettings) {
    if let Some(service) = app.try_state::<Arc<StatusRefresh>>() {
        service.configure(settings);
    }
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    let refresh = Arc::new(StatusRefresh::new());
    if let Ok(settings) = crate::app_settings::load_app_settings(app) {
        refresh.lock().configure(&settings.refresh, Instant::now());
    }
    app.manage(refresh.clone());
    let app = app.clone();
    std::thread::Builder::new()
        .name("repo-status-refresh".into())
        .spawn(move || refresh.run(app))
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(generation: u64) -> StatusSnapshot {
        StatusSnapshot {
            generation,
            completed_at: 0,
            repos: vec![],
            error: None,
        }
    }

    #[test]
    fn startup_and_concurrent_readers_share_one_scan() {
        let mut state = RefreshState::new();
        let now = Instant::now();
        assert_eq!(state.request(false), 1);
        assert_eq!(state.start(now), Some(1));
        assert_eq!(state.request(false), 1);
        assert_eq!(state.start(now), None);
        state.complete(snapshot(1), now);
        assert_eq!(state.request(false), 1);
        assert_eq!(state.start(now), None);
    }

    #[test]
    fn requests_during_collection_coalesce_into_one_followup() {
        let mut state = RefreshState::new();
        let now = Instant::now();
        assert_eq!(state.start(now), Some(1));
        for _ in 0..50 {
            assert_eq!(state.request(true), 2);
        }
        state.complete(snapshot(1), now);
        assert_eq!(state.start(now), Some(2));
        state.complete(snapshot(2), now);
        assert_eq!(state.start(now), None);
    }

    #[test]
    fn native_timer_obeys_changes_disable_and_clamping() {
        let mut state = RefreshState::new();
        let now = Instant::now();
        state.start(now);
        state.configure(
            &RefreshSettings {
                lightweight_refresh_enabled: true,
                interval_minutes: 0,
            },
            now,
        );
        state.complete(snapshot(1), now);
        assert_eq!(state.start(now + Duration::from_secs(59)), None);
        assert_eq!(state.start(now + Duration::from_secs(60)), Some(2));
        state.complete(snapshot(2), now);
        state.configure(
            &RefreshSettings {
                lightweight_refresh_enabled: false,
                interval_minutes: 5,
            },
            now,
        );
        assert_eq!(state.start(now + Duration::from_secs(3600)), None);
    }

    #[test]
    fn waiting_callers_receive_completion_and_recover_after_failure() {
        let refresh = Arc::new(StatusRefresh::new());
        let reader = refresh.clone();
        let waiter = std::thread::spawn(move || reader.wait(1));
        let mut failed = snapshot(1);
        failed.error = Some("read failed".into());
        refresh.lock().complete(failed, Instant::now());
        refresh.changed.notify_all();
        assert_eq!(waiter.join().unwrap().error.as_deref(), Some("read failed"));
        assert_eq!(refresh.request(true), 2);
        refresh.lock().complete(snapshot(2), Instant::now());
        assert!(refresh.wait(2).error.is_none());
    }
}
