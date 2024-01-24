use std::{ops::Bound, path::Path, sync::Arc};

use bytes::Bytes;
use tempfile::tempdir;
use week2_day1::harness::sync;

use self::harness::check_iter_result;

use super::*;
use crate::{
    iterators::{
        concat_iterator::SstConcatIterator, merge_iterator::MergeIterator, StorageIterator,
    },
    lsm_storage::{LsmStorageInner, LsmStorageOptions, LsmStorageState},
    table::{SsTable, SsTableBuilder, SsTableIterator},
};

fn construct_merge_iterator_over_storage(
    state: &LsmStorageState,
) -> MergeIterator<SsTableIterator> {
    let mut iters = Vec::new();
    for t in &state.l0_sstables {
        iters.push(Box::new(
            SsTableIterator::create_and_seek_to_first(state.sstables.get(t).cloned().unwrap())
                .unwrap(),
        ));
    }
    for (_, files) in &state.levels {
        for f in files {
            iters.push(Box::new(
                SsTableIterator::create_and_seek_to_first(state.sstables.get(f).cloned().unwrap())
                    .unwrap(),
            ));
        }
    }
    MergeIterator::create(iters)
}

#[test]
fn test_task1_full_compaction() {
    let dir = tempdir().unwrap();
    let storage = LsmStorageInner::open(&dir, LsmStorageOptions::default_for_week1_test()).unwrap();
    storage.put(b"0", b"v1").unwrap();
    sync(&storage);
    storage.put(b"0", b"v2").unwrap();
    storage.put(b"1", b"v2").unwrap();
    storage.put(b"2", b"v2").unwrap();
    sync(&storage);
    storage.delete(b"0").unwrap();
    storage.delete(b"2").unwrap();
    sync(&storage);
    assert_eq!(storage.state.read().l0_sstables.len(), 3);
    let mut iter = construct_merge_iterator_over_storage(&storage.state.read());
    check_iter_result(
        &mut iter,
        vec![
            (Bytes::from_static(b"0"), Bytes::from_static(b"")),
            (Bytes::from_static(b"1"), Bytes::from_static(b"v2")),
            (Bytes::from_static(b"2"), Bytes::from_static(b"")),
        ],
    );
    storage.force_full_compaction().unwrap();
    assert!(storage.state.read().l0_sstables.is_empty());
    let mut iter = construct_merge_iterator_over_storage(&storage.state.read());
    check_iter_result(
        &mut iter,
        vec![(Bytes::from_static(b"1"), Bytes::from_static(b"v2"))],
    );
    storage.put(b"0", b"v3").unwrap();
    storage.put(b"2", b"v3").unwrap();
    sync(&storage);
    storage.delete(b"1").unwrap();
    sync(&storage);
    let mut iter = construct_merge_iterator_over_storage(&storage.state.read());
    check_iter_result(
        &mut iter,
        vec![
            (Bytes::from_static(b"0"), Bytes::from_static(b"v3")),
            (Bytes::from_static(b"1"), Bytes::from_static(b"")),
            (Bytes::from_static(b"2"), Bytes::from_static(b"v3")),
        ],
    );
    storage.force_full_compaction().unwrap();
    assert!(storage.state.read().l0_sstables.is_empty());
    let mut iter = construct_merge_iterator_over_storage(&storage.state.read());
    check_iter_result(
        &mut iter,
        vec![
            (Bytes::from_static(b"0"), Bytes::from_static(b"v3")),
            (Bytes::from_static(b"2"), Bytes::from_static(b"v3")),
        ],
    );
}

fn generate_concat_sst(
    start_key: usize,
    end_key: usize,
    dir: impl AsRef<Path>,
    id: usize,
) -> SsTable {
    let mut builder = SsTableBuilder::new(128);
    for idx in start_key..end_key {
        let key = format!("{:05}", idx);
        builder.add(key.as_bytes(), b"test");
    }
    let path = dir.as_ref().join(format!("{id}.sst"));
    builder.build_for_test(path).unwrap()
}

#[test]
fn test_task2_concat_iterator() {
    let dir = tempdir().unwrap();
    let mut sstables = Vec::new();
    for i in 1..=10 {
        sstables.push(Arc::new(generate_concat_sst(
            i * 10,
            (i + 1) * 10,
            dir.path(),
            i,
        )));
    }
    for key in 0..120 {
        let iter = SstConcatIterator::create_and_seek_to_key(
            sstables.clone(),
            format!("{:05}", key).as_bytes(),
        )
        .unwrap();
        if key < 10 {
            assert!(iter.is_valid());
            assert_eq!(iter.key(), b"00010");
        } else if key >= 110 {
            assert!(!iter.is_valid());
        } else {
            assert!(iter.is_valid());
            assert_eq!(iter.key(), format!("{:05}", key).as_bytes());
        }
    }
    let iter = SstConcatIterator::create_and_seek_to_first(sstables.clone()).unwrap();
    assert!(iter.is_valid());
    assert_eq!(iter.key(), b"00010");
}

#[test]
fn test_task3_integration() {
    let dir = tempdir().unwrap();
    let storage = LsmStorageInner::open(&dir, LsmStorageOptions::default_for_week1_test()).unwrap();
    storage.put(b"0", b"2333333").unwrap();
    storage.put(b"00", b"2333333").unwrap();
    storage.put(b"4", b"23").unwrap();
    sync(&storage);

    storage.delete(b"4").unwrap();
    sync(&storage);

    storage.force_full_compaction().unwrap();
    assert!(storage.state.read().l0_sstables.is_empty());
    assert!(!storage.state.read().levels[0].1.is_empty());

    storage.put(b"1", b"233").unwrap();
    storage.put(b"2", b"2333").unwrap();
    sync(&storage);

    storage.put(b"00", b"2333").unwrap();
    storage.put(b"3", b"23333").unwrap();
    storage.delete(b"1").unwrap();
    sync(&storage);
    storage.force_full_compaction().unwrap();

    assert!(storage.state.read().l0_sstables.is_empty());
    assert!(!storage.state.read().levels[0].1.is_empty());

    check_iter_result(
        &mut storage.scan(Bound::Unbounded, Bound::Unbounded).unwrap(),
        vec![
            (Bytes::from("0"), Bytes::from("2333333")),
            (Bytes::from("00"), Bytes::from("2333")),
            (Bytes::from("2"), Bytes::from("2333")),
            (Bytes::from("3"), Bytes::from("23333")),
        ],
    );

    assert_eq!(
        storage.get(b"0").unwrap(),
        Some(Bytes::from_static(b"2333333"))
    );
    assert_eq!(
        storage.get(b"00").unwrap(),
        Some(Bytes::from_static(b"2333"))
    );
    assert_eq!(
        storage.get(b"2").unwrap(),
        Some(Bytes::from_static(b"2333"))
    );
    assert_eq!(
        storage.get(b"3").unwrap(),
        Some(Bytes::from_static(b"23333"))
    );
    assert_eq!(storage.get(b"4").unwrap(), None);
    assert_eq!(storage.get(b"--").unwrap(), None);
    assert_eq!(storage.get(b"555").unwrap(), None);
}