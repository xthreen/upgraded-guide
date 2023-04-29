// File: task_queue.rs
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc as sync_Arc, Mutex as sync_Mutex,
};
use std::time::{Duration, Instant};

use async_std::channel;
use async_std::channel::Receiver;
use async_std::task::{self, JoinHandle};
use log::debug;

pub trait Task: Send + Sync {
    fn id(&self) -> usize;
    fn poll(&mut self) -> PollResult;
    fn cancel(&mut self) -> Result<(), TaskError>;
    fn pause(&mut self) -> Result<(), TaskError>;
    fn resume(&mut self) -> Result<(), TaskError>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound,
    AlreadyRunning,
    AlreadyPaused,
    AlreadyCancelled,
    AlreadyCompleted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollingData {
    Float(f32),
    // Tuple((usize, String, f32)),
    // HashMap(HashMap<usize, (String, f32)>)
}

impl Display for PollingData {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            PollingData::Float(float_value) => write!(f, "{}", float_value),
            // PollingData::Tuple((id, name, progress)) => write!(f, "({}, {}, {})", id, name, progress),
            // PollingData::HashMap(map) => {
            //     let mut s = String::new();
            //     for (id, data) in map {
            //         s.push_str(&format!("({}, ({}, {})), ", id, data.0, data.1));
            //     }
            //     write!(f, "{}", s)
            // }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum PollResult {
    Pending(PollingData),
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Cancelled,
}

pub struct TaskQueue {
    tasks: sync_Mutex<HashMap<usize, sync_Arc<sync_Mutex<dyn Task + Send + 'static>>>>,
    next_id: AtomicUsize,
}

impl TaskQueue {
    pub fn new() -> Self {
        TaskQueue {
            tasks: sync_Mutex::new(HashMap::new()),
            next_id: AtomicUsize::new(0),
        }
    }

    pub fn add_task<T: Task + Send + 'static>(&self, task: T) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.tasks
            .lock()
            .unwrap()
            .insert(id, sync_Arc::new(sync_Mutex::new(task)));
        debug!("Added task with id: {}", id);
        id
    }

    pub fn poll_task(&self, id: usize) -> Result<PollResult, TaskError> {
        match self.tasks.lock().unwrap().get_mut(&id) {
            Some(task) => {
                let mut guard = task.lock().unwrap();
                Ok(guard.poll())
            }
            None => Err(TaskError::NotFound),
        }
    }

    pub fn _remove_task(&self, id: usize) -> Result<(), TaskError> {
        match self.tasks.lock().unwrap().remove(&id) {
            Some(_) => Ok(()),
            None => Err(TaskError::NotFound),
        }
    }

    pub fn _get_task(&self, id: usize) -> Result<Receiver<()>, TaskError> {
        debug!("Got task with id: {}", id);
        match self.tasks.lock().unwrap().get_mut(&id) {
            Some(task) => {
                debug!("matched Some(task) with id: {}", id);
                let task_clone = task.clone();
                let (tx, rx) = channel::bounded(1);
                let tx_clone = tx.clone();
                task::spawn(async move {
                    let mut guard = task_clone.lock().unwrap();
                    loop {
                        let result = guard.poll();
                        match result {
                            PollResult::Pending(progress) => {
                                debug!("PollResult::Pending: {}", progress);
                                continue;
                            }
                            PollResult::Paused => {
                                debug!("PollResult::Paused");
                                continue;
                            }
                            PollResult::Completed => {
                                debug!("PollResult::Completed");
                                break;
                            }
                            PollResult::Cancelled => {
                                debug!("PollResult::Cancelled");
                                break;
                            }
                        }
                    }
                    let _ = tx_clone.send(());
                });

                Ok(rx)
            }
            None => Err(TaskError::NotFound),
        }
    }
}

pub struct SleepTask {
    id: usize,
    duration: Duration,
    status: sync_Arc<sync_Mutex<TaskStatus>>,
    handle: Option<JoinHandle<()>>,
    start_time: sync_Arc<sync_Mutex<Option<Instant>>>,
    elapsed_time: Duration,
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
        }
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
                        *status_guard = TaskStatus::Completed;
                    }
                    debug!("SleepTask::poll() - Done sleeping");
                }));

                PollResult::Pending(PollingData::Float(0.0))
            }
            TaskStatus::Running => {
                debug!("SleepTask::poll() - Running");
                let start_time = self.start_time.lock().unwrap();
                if let Some(time) = *start_time {
                    self.elapsed_time = time.elapsed();
                    let progress = self.elapsed_time.as_secs_f32() / self.duration.as_secs_f32();
                    PollResult::Pending(PollingData::Float(progress.min(1.0)))
                } else {
                    PollResult::Pending(PollingData::Float(0.0))
                }
            }
            TaskStatus::Paused => {
                debug!("SleepTask::poll() - Paused");
                PollResult::Paused
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
            TaskStatus::Queued => Err(TaskError::NotFound),
            TaskStatus::Running => {
                {
                    let mut status_guard = self.status.lock().unwrap();
                    *status_guard = TaskStatus::Paused;
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

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger;

    fn setup_logging() {
        let _ = env_logger::Builder::new()
            .filter(None, log::LevelFilter::Debug)
            .try_init();
    }

    #[test]
    fn test_add_task() {
        let task_queue = TaskQueue::new();
        let task = SleepTask::new(0, Duration::from_millis(100));
        let task_id = task_queue.add_task(task);
        assert_eq!(task_id, 0);
    }

    #[test]
    fn test_sleep_task_completion() {
        setup_logging();
        task::block_on(async {
            let task_queue = TaskQueue::new();
            let task = SleepTask::new(0, Duration::from_millis(100));
            let task_id = task_queue.add_task(task);

            let rx = task_queue._get_task(task_id).unwrap();
            let result = async_std::future::timeout(Duration::from_secs(2), rx.recv()).await;
            assert!(
                result.is_ok(),
                "Task did not complete within the expected time"
            );

            let poll_result = task_queue.poll_task(task_id).unwrap();
            assert_eq!(poll_result, PollResult::Completed);
        });
    }

    #[test]
    fn test_add_multiple_tasks() {
        let task_queue = TaskQueue::new();

        let task_one = SleepTask::new(0, Duration::from_secs(2));
        let task_one_id = task_queue.add_task(task_one);

        let task_two = SleepTask::new(1, Duration::from_secs(2));
        let task_two_id = task_queue.add_task(task_two);

        assert_eq!(task_one_id, 0);
        assert_eq!(task_two_id, 1);
    }

    #[test]
    fn test_poll_task() {
        let task_queue = TaskQueue::new();
        let task = SleepTask::new(0, Duration::from_millis(100));
        let task_id = task_queue.add_task(task);
        let poll_result = task_queue.poll_task(task_id);
        match poll_result {
            Ok(PollResult::Pending(PollingData::Float(progress))) => {
                assert!(progress >= 0.0 && progress <= 1.0, "Progress should be a float between 0.0 and 1.0");
            }
            _ => {
                panic!("Expected PollResult::Pending");
            }
        }
    }

    #[test]
    fn test_remove_task() {
        let task_queue = TaskQueue::new();
        let task = SleepTask::new(0, Duration::from_millis(100));
        let task_id = task_queue.add_task(task);
        let remove_result = task_queue._remove_task(task_id);
        assert!(remove_result.is_ok());
    }

    #[test]
    fn test_remove_non_existent_task() {
        let task_queue = TaskQueue::new();
        let remove_result = task_queue._remove_task(0);
        assert_eq!(remove_result.unwrap_err(), TaskError::NotFound);
    }
}
