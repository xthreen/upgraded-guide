#[cfg(test)]
use crate::app::task_queue::{PollingData, PollResult, TaskQueue, TaskError};
use env_logger;

fn _setup_logging() {
    let _ = env_logger::Builder::new()
        .filter(None, log::LevelFilter::Debug)
        .try_init();
}

#[test]
fn test_add_task() {
    let task_queue = TaskQueue::new();
    let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(100));
    let task_id = task_queue.add_task(task);
    assert_eq!(task_id, 0);
}

#[test]
fn test_sleep_task_completion() {
    _setup_logging();
    async_std::task::block_on(async {
        let task_queue = TaskQueue::new();
        let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(100));
        let task_id = task_queue.add_task(task);

        let rx = task_queue._get_task(task_id).unwrap();
        let result = async_std::future::timeout(std::time::Duration::from_secs(2), rx.recv()).await;
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

    let task_one = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_secs(2));
    let task_one_id = task_queue.add_task(task_one);

    let task_two = crate::app::sleep_task::SleepTask::new(1, std::time::Duration::from_secs(2));
    let task_two_id = task_queue.add_task(task_two);

    assert_eq!(task_one_id, 0);
    assert_eq!(task_two_id, 1);
}

#[test]
fn test_poll_task() {
    let task_queue = TaskQueue::new();
    let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(100));
    let task_id = task_queue.add_task(task);
    let poll_result = task_queue.poll_task(task_id);
    match poll_result {
        Ok(PollResult::Pending(PollingData::Float(progress))) => {
            assert!(
                progress >= 0.0 && progress <= 1.0,
                "Progress should be a float between 0.0 and 1.0"
            );
        }
        _ => {
            panic!("Expected PollResult::Pending");
        }
    }
}

#[test]
fn test_remove_task() {
    _setup_logging();
    let task_queue = TaskQueue::new();
    let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(100));
    let task_id = task_queue.add_task(task);
    let remove_result = task_queue._remove_task(task_id);
    assert!(remove_result.is_ok());
}

#[test]
fn test_remove_polled_task() {
    _setup_logging();
    let task_queue = TaskQueue::new();
    let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(200));
    let task_id = task_queue.add_task(task);
    let poll_result = task_queue.poll_task(task_id);
    assert!(poll_result.is_ok());
    let remove_result = task_queue._remove_task(task_id);
    assert!(remove_result.is_ok());
}

#[test]
fn test_remove_non_existent_task() {
    let task_queue = TaskQueue::new();
    let remove_result = task_queue._remove_task(0);
    assert_eq!(remove_result.unwrap_err(), TaskError::NotFound);
}

#[test]
fn test_pause_and_resume() {
    _setup_logging();
    async_std::task::block_on(async {
        let task_queue = TaskQueue::new();
        let task = crate::app::sleep_task::SleepTask::new(0, std::time::Duration::from_millis(500));
        let task_id = task_queue.add_task(task);

        let poll_result = task_queue.poll_task(task_id).unwrap();
        assert_eq!(poll_result, PollResult::Pending(PollingData::Float(0.0)));

        async_std::task::sleep(std::time::Duration::from_millis(100)).await;

        let pause_result = task_queue.pause_task(task_id);
        assert!(pause_result.is_ok());

        let poll_result = task_queue.poll_task(task_id).unwrap();
        match poll_result {
            PollResult::Paused(PollingData::Float(progress)) => {
                assert!(
                    progress >= 0.0 && progress <= 1.0,
                    "Progress should be a float between 0.0 and 1.0"
                )
            }
            _ => panic!("Expected PollResult::Paused"),
        }

        let progress_before_pause = match poll_result {
            PollResult::Paused(PollingData::Float(f)) => f,
            _ => panic!("Expected PollResult::Paused"),
        };

        async_std::task::sleep(std::time::Duration::from_millis(100)).await;

        let poll_result = task_queue.poll_task(task_id).unwrap();
        assert_eq!(
            poll_result,
            PollResult::Paused(PollingData::Float(progress_before_pause))
        );

        let resume_result = task_queue.resume_task(task_id);
        assert!(resume_result.is_ok());

        async_std::task::sleep(std::time::Duration::from_millis(300)).await;

        let poll_result = task_queue.poll_task(task_id).unwrap();
        assert_eq!(poll_result, PollResult::Completed);
    });
}