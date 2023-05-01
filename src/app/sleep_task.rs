use async_std::task::{self, JoinHandle};
use log::debug;
use std::sync::{Arc as sync_Arc, Mutex as sync_Mutex};
use std::time::{Duration, Instant};

use crate::app::task_queue::PollingData;

use super::task_queue::{PollResult, Task, TaskError, TaskStatus};

pub struct SleepTask {
    id: usize,
    duration: Duration,
    status: sync_Arc<sync_Mutex<TaskStatus>>,
    handle: Option<JoinHandle<()>>,
    start_time: sync_Arc<sync_Mutex<Option<Instant>>>,
    elapsed_time: Duration,
    paused_duration: sync_Arc<sync_Mutex<Duration>>,
}

impl SleepTask {
    pub fn new(id: usize, duration: Duration) -> Self {
        SleepTask {
            id,
            duration,
            status: sync_Arc::new(sync_Mutex::new(TaskStatus::Queued)),
            handle: None,
            start_time: sync_Arc::new(sync_Mutex::new(None)),
            elapsed_time: Duration::from_secs(0),
            paused_duration: sync_Arc::new(sync_Mutex::new(Duration::from_secs(0))),
        }
    }

    pub fn kind(&self) -> &str {
        "SleepTask"
    }
}

impl Task for SleepTask {
    fn id(&self) -> usize {
        self.id
    }

    fn poll(self: &mut SleepTask) -> PollResult {
        let status = self.status.lock().unwrap().clone();
        match status {
            TaskStatus::Queued => {
                debug!("SleepTask::poll() - Queued");
                let duration = self.duration;
                let shared_status = self.status.clone();
                let shared_start_time = self.start_time.clone();
                self.handle = Some(task::spawn(async move {
                    debug!("SleepTask::poll() - Sleeping for {:?}", duration);
                    {
                        let mut status_guard = shared_status.lock().unwrap();
                        *status_guard = TaskStatus::Running;
                    }
                    {
                        let mut start_time_guard = shared_start_time.lock().unwrap();
                        *start_time_guard = Some(Instant::now());
                    }
                    task::sleep(duration).await;
                    {
                        let mut status_guard = shared_status.lock().unwrap();
                        match status_guard.clone() {
                            TaskStatus::Running => {
                                *status_guard = TaskStatus::Completed;
                            }
                            TaskStatus::Paused => {
                                *status_guard = TaskStatus::Paused;
                            }
                            _ => {}
                        }
                    }
                    debug!("SleepTask::poll() - Done sleeping");
                }));

                PollResult::Pending(PollingData::Float(0.0))
            }
            TaskStatus::Running => {
                debug!("SleepTask::poll() - Running");
                let start_time = self.start_time.lock().unwrap();
                let paused_duration = self.paused_duration.lock().unwrap();
                if let Some(time) = *start_time {
                    if *paused_duration > Duration::from_secs(0) {
                        log::debug!(
                            "{}: paused_duration: {:?}, elapsed_time: {:?}",
                            self.kind(),
                            paused_duration,
                            self.elapsed_time
                        );
                        self.elapsed_time = time.elapsed() + *paused_duration;
                        let progress =
                            self.elapsed_time.as_secs_f32() / self.duration.as_secs_f32();
                        if progress >= 1.0 {
                            PollResult::Completed
                        } else {
                            PollResult::Pending(PollingData::Float(progress.min(1.0)))
                        }
                    } else {
                        self.elapsed_time = time.elapsed();
                        let progress =
                            self.elapsed_time.as_secs_f32() / self.duration.as_secs_f32();
                        PollResult::Pending(PollingData::Float(progress.min(1.0)))
                    }
                } else {
                    PollResult::Pending(PollingData::Float(0.0))
                }
            }
            TaskStatus::Paused => {
                debug!("SleepTask::poll() - Paused");
                let paused_duration = self.paused_duration.lock().unwrap();
                log::debug!(
                    "{}: paused task {} paused_duration: {:?}",
                    self.kind(),
                    self.id(),
                    paused_duration
                );
                let progress = paused_duration.as_secs_f32() / self.duration.as_secs_f32();
                PollResult::Paused(PollingData::Float(progress.min(1.0)))
            }
            TaskStatus::Completed => {
                debug!("SleepTask::poll() - Completed");
                PollResult::Completed
            }
            TaskStatus::Cancelled => {
                debug!("SleepTask::poll() - Cancelled");
                PollResult::Cancelled
            }
        }
    }

    fn cancel(self: &mut SleepTask) -> Result<(), TaskError> {
        let status = self.status.lock().unwrap().clone();
        match status {
            TaskStatus::Queued => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Cancelled;
                }
                Ok(())
            }
            TaskStatus::Running => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Cancelled;
                }
                Ok(())
            }
            TaskStatus::Paused => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Cancelled;
                }
                Ok(())
            }
            TaskStatus::Completed => Err(TaskError::AlreadyCompleted),
            TaskStatus::Cancelled => Err(TaskError::AlreadyCancelled),
        }
    }

    fn pause(self: &mut SleepTask) -> Result<(), TaskError> {
        let status = self.status.lock().unwrap().clone();
        match status {
            TaskStatus::Queued => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Paused;
                }
                Ok(())
            }
            TaskStatus::Running => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Paused;
                }
                {
                    let paused_at = Instant::now();
                    let start_time = self.start_time.lock().unwrap();
                    let diff = paused_at.duration_since(start_time.unwrap());
                    log::debug!("pausing {} task {}", self.kind(), self.id());
                    let mut paused_duration_guard = self.paused_duration.lock().unwrap();
                    *paused_duration_guard = diff;
                }
                Ok(())
            }
            TaskStatus::Paused => Err(TaskError::AlreadyPaused),
            TaskStatus::Completed => Err(TaskError::AlreadyCompleted),
            TaskStatus::Cancelled => Err(TaskError::AlreadyCancelled),
        }
    }

    fn resume(self: &mut SleepTask) -> Result<(), TaskError> {
        let status = self.status.lock().unwrap().clone();
        match status {
            TaskStatus::Queued => Err(TaskError::NotFound),
            TaskStatus::Running => Err(TaskError::AlreadyRunning),
            TaskStatus::Paused => {
                {
                    let mut resume_time_guard = self.start_time.lock().unwrap();
                    *resume_time_guard = Some(Instant::now());
                }
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Running;
                }
                Ok(())
            }
            TaskStatus::Completed => Err(TaskError::AlreadyCompleted),
            TaskStatus::Cancelled => Err(TaskError::AlreadyCancelled),
        }
    }
}
