//! GC 測試：驗證 mark-and-sweep 垃圾回收器的基本功能。
//!
//! 測試項目：
//! - 分配大量物件後 GC 能正確回收不可達物件
//! - 有根引用的可達物件不會被回收
//! - 被釋放的 slot 能被重新使用

use tiny_lang::environment::Value;
use tiny_lang::gc::GcHeap;

#[test]
fn gc_collects_all_unreachable() {
    let mut heap = GcHeap::new();

    // 分配 50 個字串物件（無任何根引用）
    for i in 0..50 {
        heap.alloc_string(format!("string_{i}"));
    }
    assert_eq!(heap.current_heap_size(), 50);

    // 以空根列表執行 GC，所有物件應被回收
    heap.mark_and_sweep(&[], &[]);

    assert_eq!(heap.current_heap_size(), 0, "所有不可達物件應被回收");
    assert_eq!(heap.total_collections, 1);
}

#[test]
fn gc_preserves_reachable_roots() {
    let mut heap = GcHeap::new();

    // 分配兩個「根」字串和十個「垃圾」字串
    let root1 = heap.alloc_string("keep_me".into());
    let root2 = heap.alloc_string("keep_me_too".into());
    for i in 0..10 {
        heap.alloc_string(format!("garbage_{i}"));
    }
    assert_eq!(heap.current_heap_size(), 12);

    // 以 root1、root2 作為根執行 GC
    let roots = vec![
        Value::String(root1.clone()),
        Value::String(root2.clone()),
    ];
    heap.mark_and_sweep(&roots, &[]);

    // 只有根引用的物件應該存活
    assert_eq!(heap.current_heap_size(), 2, "只有可達物件應存活");
    assert_eq!(heap.get_string(&root1), "keep_me");
    assert_eq!(heap.get_string(&root2), "keep_me_too");
}

#[test]
fn gc_reuses_freed_slots() {
    let mut heap = GcHeap::new();

    // 分配後立即回收
    heap.alloc_string("temp1".into());
    heap.alloc_string("temp2".into());
    heap.mark_and_sweep(&[], &[]);
    assert_eq!(heap.current_heap_size(), 0);

    // 記錄已分配數量
    let alloc_before = heap.total_allocations;

    // 再次分配，應重用已釋放的 slot
    let new_ref = heap.alloc_string("reused".into());
    assert_eq!(heap.total_allocations, alloc_before + 1);
    assert_eq!(heap.current_heap_size(), 1);
    // 分配後大小不應超過 2（重用 slot，不追加新條目）
    // 內部陣列長度不超過原始大小（2）
    assert_eq!(heap.get_string(&new_ref), "reused");
}

#[test]
fn gc_threshold_should_collect() {
    let mut heap = GcHeap::new();

    // 預設閾值為 1024，初始不應觸發 GC
    assert!(!heap.should_collect(), "初始堆大小為 0，不應觸發 GC");

    // 分配到閾值前一個
    for _ in 0..1023 {
        heap.alloc_string("test".into());
    }
    assert!(!heap.should_collect(), "1023 個物件不應觸發 GC");

    // 分配第 1024 個，達到閾值
    heap.alloc_string("trigger".into());
    assert!(heap.should_collect(), "1024 個物件應觸發 GC");
}

#[test]
fn gc_gc_stats_track_correctly() {
    let mut heap = GcHeap::new();

    heap.alloc_string("a".into());
    heap.alloc_string("b".into());
    heap.alloc_string("c".into());

    let stats_before = heap.stats();
    assert_eq!(stats_before.total_allocations, 3);
    assert_eq!(stats_before.total_collections, 0);
    assert_eq!(stats_before.current_heap_size, 3);

    // GC 後統計更新
    heap.mark_and_sweep(&[], &[]);
    let stats_after = heap.stats();
    assert_eq!(stats_after.total_collections, 1);
    assert_eq!(stats_after.current_heap_size, 0);
}
