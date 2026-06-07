use super::*;

#[test]
fn task_details_new_creates_empty() {
    let details = TaskDetails::new();
    assert!(details.task_ids().is_empty());
    assert!(details.task_names().is_empty());
}

#[test]
fn task_details_insert_and_get() {
    let mut details = TaskDetails::new();
    details.insert(1, "task_one".to_string());
    details.insert(2, "task_two".to_string());

    assert_eq!(details.get(&1), Some(&"task_one".to_string()));
    assert_eq!(details.get(&2), Some(&"task_two".to_string()));
    assert_eq!(details.get(&3), None);
}

#[test]
fn task_details_task_ids_and_names() {
    let mut details = TaskDetails::new();
    details.insert(10, "alpha".to_string());
    details.insert(20, "beta".to_string());

    let ids = details.task_ids();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&10));
    assert!(ids.contains(&20));

    let names = details.task_names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"alpha".to_string()));
    assert!(names.contains(&"beta".to_string()));
}

#[test]
fn task_details_get_or_empty_found() {
    let mut details = TaskDetails::new();
    details.insert(5, "my_task".to_string());

    let result = details.get_or_empty(&100, &5);
    assert_eq!(result, "my_task");
}

#[test]
fn task_details_get_id_found() {
    let mut details = TaskDetails::new();
    details.insert(5, "my_task".to_string());

    assert_eq!(details.get_id("my_task"), Some(5));
    assert_eq!(details.get_id("other_task"), None);
}

#[test]
fn task_details_get_or_empty_not_found() {
    let details = TaskDetails::new();
    let result = details.get_or_empty(&100, &999);
    assert_eq!(result, "");
}

#[test]
fn task_details_default() {
    let details = TaskDetails::default();
    assert!(details.task_ids().is_empty());
}

#[tokio::test]
async fn shared_task_details_new_and_default() {
    let shared = SharedTaskDetails::new(TaskDetails::new());
    assert!(shared.task_ids().await.is_empty());

    let shared_default = SharedTaskDetails::default();
    assert!(shared_default.task_ids().await.is_empty());
}

#[tokio::test]
async fn shared_task_details_from_task_details() {
    let mut details = TaskDetails::new();
    details.insert(1, "test".to_string());

    let shared: SharedTaskDetails = details.into();
    assert_eq!(shared.get(&1).await, Some("test".to_string()));
}

#[tokio::test]
async fn shared_task_details_insert_and_get() {
    let shared = SharedTaskDetails::default();
    shared.insert(42, "answer".to_string()).await;

    assert_eq!(shared.get(&42).await, Some("answer".to_string()));
    assert_eq!(shared.get(&0).await, None);
}

#[tokio::test]
async fn shared_task_details_task_ids_and_names() {
    let shared = SharedTaskDetails::default();
    shared.insert(1, "one".to_string()).await;
    shared.insert(2, "two".to_string()).await;

    let ids = shared.task_ids().await;
    assert_eq!(ids.len(), 2);

    let names = shared.task_names().await;
    assert_eq!(names.len(), 2);
}

#[tokio::test]
async fn shared_task_details_get_or_empty() {
    let shared = SharedTaskDetails::default();
    shared.insert(5, "found".to_string()).await;

    assert_eq!(shared.get_or_empty(&1, &5).await, "found");
    assert_eq!(shared.get_or_empty(&1, &999).await, "");
}

#[tokio::test]
async fn shared_task_details_read_write() {
    let shared = SharedTaskDetails::default();

    {
        let mut guard = shared.write().await;
        guard.insert(100, "via_write".to_string());
    }

    {
        let guard = shared.read().await;
        assert_eq!(guard.get(&100), Some(&"via_write".to_string()));
    }
}
