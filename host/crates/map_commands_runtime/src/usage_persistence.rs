use std::collections::VecDeque;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use core_types::HolonError;

/// The outcome of one attempt, preserving whether the transaction may be resumed.
pub(crate) enum SaveOutcome {
    Complete,
    Incomplete,
}

/// A retained logical save. Retrying must resume this object, not create a new transaction.
pub(crate) trait UsageSave: Send + std::any::Any {
    fn attempt(&mut self) -> Result<SaveOutcome, HolonError>;
    /// Whether ordering can be affected by this other save. Unknown save kinds are barriers.
    fn conflicts(&self, _other: &dyn UsageSave) -> bool {
        true
    }
    /// Absorb an unstarted successor without backend work. False leaves both saves unchanged.
    fn coalesce(&mut self, _newer: &mut dyn UsageSave) -> bool {
        false
    }
    /// Dependencies are in-memory save revisions, never backend availability probes.
    fn is_ready(&self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SaveStatus {
    Queued,
    Saving,
    Saved,
    Failed(String),
    ServicePaused(String),
}

struct Job {
    ids: Vec<u64>,
    save: Box<dyn UsageSave>,
    attempts: usize,
    ready: Instant,
}

#[derive(Default)]
struct State {
    next_id: u64,
    queue: VecDeque<Job>,
    retained: std::collections::HashMap<u64, Job>,
    statuses: std::collections::HashMap<u64, SaveStatus>,
    service_paused: bool,
    stopped: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

/// Session-local queue. Its worker never holds the admission lock during backend work.
pub(crate) struct UsagePersistence {
    shared: Arc<Shared>,
}

impl Default for UsagePersistence {
    fn default() -> Self {
        let shared =
            Arc::new(Shared { state: Mutex::new(State::default()), changed: Condvar::new() });
        let worker = Arc::clone(&shared);
        std::thread::Builder::new()
            .name("usage-persistence".into())
            .spawn(move || {
                // Websocket requests use Tokio timers even when driven through the
                // synchronous reference facade. Give this worker its own runtime.
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("Usage persistence runtime");
                let entered = runtime.enter();
                run(worker);
                drop(entered);
                runtime.shutdown_background();
            })
            .expect("start Usage persistence worker");
        Self { shared }
    }
}

impl Drop for UsagePersistence {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            state.stopped = true;
            state.queue.clear();
            state.retained.clear();
            self.shared.changed.notify_all();
        }
        // Application shutdown does not wait for backend I/O.
    }
}

impl UsagePersistence {
    /// Acceptance transfers ownership before returning, independently of worker availability.
    pub(crate) fn submit(&self, mut save: Box<dyn UsageSave>) -> Result<u64, HolonError> {
        let mut state = self.shared.state.lock().map_err(lock_error)?;
        state.next_id += 1;
        let id = state.next_id;
        state.statuses.insert(id, SaveStatus::Queued);
        // Disjoint batches may be skipped, but never cross a shared Usage or
        // preference dependency. In-flight, failed and retried transactions retain
        // their original snapshots and retry budgets.
        for previous in state.queue.iter_mut().rev() {
            if !previous.save.conflicts(save.as_ref()) {
                continue;
            }
            if previous.attempts == 0 && previous.save.coalesce(save.as_mut()) {
                previous.ids.push(id);
                self.shared.changed.notify_one();
                return Ok(id);
            }
            break;
        }
        state.queue.push_back(Job { ids: vec![id], save, attempts: 0, ready: Instant::now() });
        self.shared.changed.notify_one();
        Ok(id)
    }

    pub(crate) fn status(&self, id: u64) -> Result<SaveStatus, HolonError> {
        self.shared
            .state
            .lock()
            .map_err(lock_error)?
            .statuses
            .get(&id)
            .cloned()
            .ok_or_else(|| HolonError::InvalidParameter("Unknown Usage persistence batch".into()))
    }

    /// Explicit retry resumes retained work and releases an outage pause without probing.
    pub(crate) fn retry(&self, id: u64) -> Result<(), HolonError> {
        let mut state = self.shared.state.lock().map_err(lock_error)?;
        let retained_id =
            state.retained.iter().find(|(_, job)| job.ids.contains(&id)).map(|(key, _)| *key);
        let mut job = retained_id.and_then(|key| state.retained.remove(&key)).ok_or_else(|| {
            HolonError::InvalidState("Usage batch has no failed save to retry".into())
        })?;
        job.attempts = 0;
        job.ready = Instant::now();
        state.service_paused = false;
        set_status(&mut state, &job, SaveStatus::Queued);
        state.queue.push_front(job);
        self.shared.changed.notify_one();
        Ok(())
    }
}

fn set_status(state: &mut State, job: &Job, status: SaveStatus) {
    for id in &job.ids {
        state.statuses.insert(*id, status.clone());
    }
}

fn lock_error(error: impl std::fmt::Display) -> HolonError {
    HolonError::FailedToAcquireLock(error.to_string())
}

fn run(shared: Arc<Shared>) {
    loop {
        let mut state = shared.state.lock().expect("Usage queue lock");
        let mut job = loop {
            if state.stopped {
                return;
            }
            if !state.service_paused {
                if let Some(index) = state
                    .queue
                    .iter()
                    .position(|job| job.ready <= Instant::now() && job.save.is_ready())
                {
                    break state.queue.remove(index).expect("queued job");
                }
            }
            let delay = state
                .queue
                .iter()
                .filter(|job| job.save.is_ready())
                .map(|job| job.ready.saturating_duration_since(Instant::now()))
                .min();
            state = if state.service_paused || delay.is_none() {
                shared.changed.wait(state).expect("Usage queue lock")
            } else {
                shared.changed.wait_timeout(state, delay.unwrap()).expect("Usage queue lock").0
            };
        };
        set_status(&mut state, &job, SaveStatus::Saving);
        drop(state);

        job.attempts += 1;
        let outcome = job.save.attempt();

        let mut state = shared.state.lock().expect("Usage queue lock");
        if state.stopped {
            return;
        }
        match outcome {
            Ok(SaveOutcome::Complete) => {
                set_status(&mut state, &job, SaveStatus::Saved);
            }
            // An incomplete response is explicit lifecycle evidence that the SAME
            // retained transaction may resume. Unknown transport errors are not.
            Ok(SaveOutcome::Incomplete) if job.attempts < 3 => {
                job.ready = Instant::now() + Duration::from_millis(250 * job.attempts as u64);
                set_status(&mut state, &job, SaveStatus::Queued);
                state.queue.push_back(job);
            }
            other => {
                let status = match other {
                    Err(HolonError::ServiceNotAvailable(reason)) => {
                        state.service_paused = true;
                        SaveStatus::ServicePaused(reason)
                    }
                    Err(error) => SaveStatus::Failed(error.to_string()),
                    _ => SaveStatus::Failed(
                        "Incomplete Usage commit exhausted automatic retries".into(),
                    ),
                };
                set_status(&mut state, &job, status);
                state.retained.insert(job.ids[0], job);
            }
        }
        shared.changed.notify_all();
    }
}
