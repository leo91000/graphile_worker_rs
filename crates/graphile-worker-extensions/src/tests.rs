use super::*;

#[test]
fn test_insert_and_get() {
    let mut ext = Extensions::new();
    assert!(ext.insert(5i32).is_none());
    assert_eq!(ext.get::<i32>(), Some(&5i32));
}

#[test]
fn test_new_starts_empty() {
    let ext = Extensions::new();
    assert!(ext.is_empty());
    assert_eq!(ext.len(), 0);
    assert!(ext.get::<i32>().is_none());
}

#[test]
fn test_insert_returns_replaced_value() {
    let mut ext = Extensions::new();
    assert!(ext.insert(5i32).is_none());
    assert_eq!(ext.insert(7i32), Some(5i32));
    assert_eq!(ext.get::<i32>(), Some(&7i32));
}

#[test]
fn test_insert_and_get_mut() {
    let mut ext = Extensions::new();
    ext.insert(String::from("Hello"));
    ext.get_mut::<String>().unwrap().push_str(" World");
    assert_eq!(ext.get::<String>().unwrap(), "Hello World");
}

#[test]
fn test_get_mut_missing_returns_none() {
    let mut ext = Extensions::new();
    assert!(ext.get_mut::<String>().is_none());
}

#[test]
fn test_get_or_insert() {
    let mut ext = Extensions::new();
    *ext.get_or_insert(1i32) += 2;
    assert_eq!(ext.get::<i32>(), Some(&3i32));
}

#[test]
fn test_get_or_insert_with() {
    let mut ext = Extensions::new();
    *ext.get_or_insert_with(|| 1i32) += 2;
    assert_eq!(ext.get::<i32>(), Some(&3i32));
}

#[test]
fn test_get_or_insert_with_keeps_existing_value() {
    let mut ext = Extensions::new();
    ext.insert(1i32);
    *ext.get_or_insert_with(|| 9i32) += 2;
    assert_eq!(ext.get::<i32>(), Some(&3i32));
}

#[test]
fn test_get_or_insert_default() {
    let mut ext = Extensions::new();
    *ext.get_or_insert_default::<i32>() += 2;
    assert_eq!(ext.get::<i32>(), Some(&2i32));
}

#[test]
fn test_remove() {
    let mut ext = Extensions::new();
    ext.insert(5i32);
    assert_eq!(ext.remove::<i32>(), Some(5i32));
    assert!(ext.get::<i32>().is_none());
}

#[test]
fn test_clear() {
    let mut ext = Extensions::new();
    ext.insert(5i32);
    ext.clear();
    assert!(ext.get::<i32>().is_none());
}

#[test]
fn test_is_empty() {
    let mut ext = Extensions::new();
    assert!(ext.is_empty());
    ext.insert(5i32);
    assert!(!ext.is_empty());
}

#[test]
fn test_len() {
    let mut ext = Extensions::new();
    assert_eq!(ext.len(), 0);
    ext.insert(5i32);
    assert_eq!(ext.len(), 1);
}

#[test]
fn test_extend() {
    let mut ext_a = Extensions::new();
    ext_a.insert(8u8);
    ext_a.insert(16u16);

    let mut ext_b = Extensions::new();
    ext_b.insert(4u8);
    ext_b.insert("hello");

    ext_a.extend(ext_b);
    assert_eq!(ext_a.len(), 3);
    assert_eq!(ext_a.get::<u8>(), Some(&4u8));
    assert_eq!(ext_a.get::<u16>(), Some(&16u16));
    assert_eq!(ext_a.get::<&'static str>().copied(), Some("hello"));
}

#[test]
fn test_extend_into_empty() {
    let mut ext_a = Extensions::new();
    let mut ext_b = Extensions::new();
    ext_b.insert(4u8);

    ext_a.extend(ext_b);

    assert_eq!(ext_a.get::<u8>(), Some(&4u8));
}

#[test]
fn test_clone_and_read_only_extensions() {
    let mut ext = Extensions::new();
    ext.insert(String::from("hello"));

    let cloned = ext.clone();
    assert_eq!(cloned.get::<String>().unwrap(), "hello");

    let read_only = ReadOnlyExtensions::from(cloned);
    assert_eq!(read_only.get::<String>().unwrap(), "hello");
    assert_eq!(read_only.len(), 1);
    assert!(!read_only.is_empty());
}
