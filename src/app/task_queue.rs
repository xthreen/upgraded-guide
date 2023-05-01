use std::collections::HashMap;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc as sync_Arc, Mutex as sync_Mutex,
};

use async_std::channel;
use async_std::channel::Receiver;
use async_std::task;
use log::debug;

pub trait Task: Send + Sync {
    fn id(&self) -> Result<usize, TaskError>;
    fn set_id(&mut self, id: usize);
    fn poll(&mut self) -> PollResult;
    fn cancel(&mut self) -> Result<(), TaskError>;
    fn pause(&mut self) -> Result<(), TaskError>;
    fn resume(&mut self) -> Result<(), TaskError>;
    fn kind(&self) -> TaskKind;
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound,
    AlreadyRunning,
    AlreadyPaused,
    AlreadyCancelled,
    AlreadyCompleted,
    IdUsizeIsNone,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskKind {
    Sleep,
    // Download,
    // Process,
}

impl Display for TaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            TaskKind::Sleep => write!(f, "Sleep task"),
            // TaskKind::Download => write!(f, "Download task"),
            // TaskKind::Process => write!(f, "Process task"),
        }
    }
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
    Paused(PollingData),
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

    pub fn add_task<T: Task + Send + 'static>(&self, mut task: T) -> usize {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        task.set_id(id);
        self.tasks
            .lock()
            .expect("Panicked at add_task: Tasks mutex poisoned")
            .insert(id, sync_Arc::new(sync_Mutex::new(task)));
        debug!("Added task with id: {}", id);
        id
    }

    pub fn poll_task(&self, id: usize) -> Result<PollResult, TaskError> {
        match self
            .tasks
            .lock()
            .expect("Panicked at poll_task: Tasks mutex poisoned")
            .get_mut(&id)
        {
            Some(task) => {
                let mut guard = task.lock().unwrap();
                Ok(guard.poll())
            }
            None => Err(TaskError::NotFound),
        }
    }

    pub fn remove_task(&self, id: usize) -> Result<(), TaskError> {
        match self
            .tasks
            .lock()
            .expect("Panicked at remove_task: Tasks mutex poisoned")
            .get_mut(&id)
        {
            Some(task) => {
                let mut guard = task.lock().unwrap();
                guard.cancel()
            }
            None => Err(TaskError::NotFound),
        }
    }

    pub fn pause_task(&self, id: usize) -> Result<(), TaskError> {
        let tasks = self
            .tasks
            .lock()
            .expect("Panicked at pause_task: Tasks mutex poisoned");
        match tasks.get(&id) {
            Some(task) => {
                let mut guard = task
                    .lock()
                    .expect("Panicked unwrapping task to pause: Task mutex poisoned");
                guard.pause()
            }
            None => {
                log::error!("Task not found: {}", id);
                Err(TaskError::NotFound)
            }
        }
    }

    pub fn resume_task(&self, id: usize) -> Result<(), TaskError> {
        log::debug!("Resume requested for {}", &id);
        let tasks = self
            .tasks
            .lock()
            .expect("Panicked at resume_task: Tasks mutex poisoned");
        match tasks.get(&id) {
            Some(task) => {
                let mut guard = task
                    .lock()
                    .expect("Panicked unwrapping task to resume: Task mutex poisoned");
                log::debug!("Resumed task {}", &id);
                guard.resume()
            }
            None => {
                log::error!("Task not found: {}", id);
                Err(TaskError::NotFound)
            }
        }
    }

    pub fn _get_task(&self, id: usize) -> Result<Receiver<()>, TaskError> {
        debug!("Got task with id: {}", id);
        match self.tasks.lock().unwrap().get_mut(&id) {
            Some(task) => {
                debug!("matched Some(task) with id: {}", id);
                let task_clone = task.clone();
                let (tx, rx) = channel::bounded(1);
                let tx_clone = tx;
                task::spawn(async move {
                    let mut guard = task_clone.lock().unwrap();
                    loop {
                        let result = guard.poll();
                        match result {
                            PollResult::Pending(progress) => {
                                debug!("PollResult::Pending: {}", progress);
                                continue;
                            }
                            PollResult::Paused(p) => {
                                debug!("PollResult::Paused at {}", p);
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
                    let send = tx_clone.send(());
                    drop(send);
                });

                Ok(rx)
            }
            None => Err(TaskError::NotFound),
        }
    }
}
