use liquide_plugin_abi::types::ResourceHandle;

use crate::plugin::PluginId;
use crate::resources::ResourcePool;

#[test]
fn pool_create() {
    let pool = ResourcePool::new(1024);
    assert_eq!(pool.max_capacity(), 1024);
    assert_eq!(pool.total_allocated(), 0);
    assert_eq!(pool.available(), 1024);
    assert_eq!(pool.allocation_count(), 0);
}

#[test]
fn pool_allocate() {
    let mut pool = ResourcePool::new(1024);
    let handle = pool.allocate(256, PluginId(1)).unwrap();
    assert_eq!(pool.total_allocated(), 256);
    assert_eq!(pool.available(), 768);
    assert_eq!(pool.allocation_count(), 1);

    let alloc = pool.get(handle).unwrap();
    assert_eq!(alloc.size, 256);
    assert_eq!(alloc.owner, PluginId(1));
}

#[test]
fn pool_allocate_multiple() {
    let mut pool = ResourcePool::new(1024);
    let h1 = pool.allocate(100, PluginId(1)).unwrap();
    let h2 = pool.allocate(200, PluginId(2)).unwrap();
    let h3 = pool.allocate(300, PluginId(1)).unwrap();
    assert_eq!(pool.total_allocated(), 600);
    assert_eq!(pool.allocation_count(), 3);
    assert_ne!(h1, h2);
    assert_ne!(h2, h3);
}

#[test]
fn pool_allocate_exhausted() {
    let mut pool = ResourcePool::new(100);
    pool.allocate(80, PluginId(1)).unwrap();
    let result = pool.allocate(50, PluginId(2));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("exhausted"));
}

#[test]
fn pool_allocate_exact_capacity() {
    let mut pool = ResourcePool::new(100);
    let handle = pool.allocate(100, PluginId(1)).unwrap();
    assert_eq!(pool.available(), 0);
    assert!(pool.get(handle).is_some());
}

#[test]
fn pool_free() {
    let mut pool = ResourcePool::new(1024);
    let handle = pool.allocate(256, PluginId(1)).unwrap();
    let alloc = pool.free(handle).unwrap();
    assert_eq!(alloc.size, 256);
    assert_eq!(pool.total_allocated(), 0);
    assert_eq!(pool.allocation_count(), 0);
    assert!(pool.get(handle).is_none());
}

#[test]
fn pool_free_unknown_handle() {
    let mut pool = ResourcePool::new(1024);
    let result = pool.free(ResourceHandle(999));
    assert!(result.is_err());
}

#[test]
fn pool_free_all_for_plugin() {
    let mut pool = ResourcePool::new(10_000);
    pool.allocate(100, PluginId(1)).unwrap();
    pool.allocate(200, PluginId(1)).unwrap();
    pool.allocate(300, PluginId(2)).unwrap();
    assert_eq!(pool.total_allocated(), 600);

    let freed = pool.free_all_for_plugin(PluginId(1));
    assert_eq!(freed, 2);
    assert_eq!(pool.total_allocated(), 300);
    assert_eq!(pool.allocation_count(), 1);
}

#[test]
fn pool_free_all_for_nonexistent_plugin() {
    let mut pool = ResourcePool::new(1024);
    pool.allocate(100, PluginId(1)).unwrap();
    let freed = pool.free_all_for_plugin(PluginId(999));
    assert_eq!(freed, 0);
    assert_eq!(pool.total_allocated(), 100);
}

#[test]
fn pool_allocations_for_plugin() {
    let mut pool = ResourcePool::new(10_000);
    pool.allocate(100, PluginId(1)).unwrap();
    pool.allocate(200, PluginId(2)).unwrap();
    pool.allocate(300, PluginId(1)).unwrap();

    let allocs = pool.allocations_for_plugin(PluginId(1));
    assert_eq!(allocs.len(), 2);

    let total: u64 = allocs.iter().map(|a| a.size).sum();
    assert_eq!(total, 400);
}

#[test]
fn pool_zero_size_allocation() {
    let mut pool = ResourcePool::new(1024);
    let handle = pool.allocate(0, PluginId(1)).unwrap();
    assert_eq!(pool.total_allocated(), 0);
    assert!(pool.get(handle).is_some());
}

#[test]
fn pool_display() {
    let mut pool = ResourcePool::new(1024);
    pool.allocate(256, PluginId(1)).unwrap();
    let s = format!("{pool}");
    assert!(s.contains("ResourcePool"));
    assert!(s.contains("256/1024B"));
    assert!(s.contains("1 allocs"));
}

#[test]
fn allocation_display() {
    let mut pool = ResourcePool::new(1024);
    let handle = pool.allocate(512, PluginId(7)).unwrap();
    let alloc = pool.get(handle).unwrap();
    let s = format!("{alloc}");
    assert!(s.contains("512B"));
    assert!(s.contains("Plugin(7)"));
}
