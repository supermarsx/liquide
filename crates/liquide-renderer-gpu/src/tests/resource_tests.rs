use crate::resource::*;

#[test]
fn allocator_new_has_zero_allocations() {
    let alloc = VramAllocator::new(VramBudget::default());
    assert_eq!(alloc.allocation_count(), 0);
    assert_eq!(alloc.usage_pct(), 0.0);
}

#[test]
fn allocate_within_budget() {
    let budget = VramBudget {
        total_mb: 4096,
        allocated_mb: 0,
        session_budget_mb: 256,
    };
    let mut alloc = VramAllocator::new(budget);

    let id = alloc
        .allocate(AllocationPurpose::TextureAtlas, 64 * 1024 * 1024)
        .expect("should succeed within budget");

    assert_eq!(alloc.allocation_count(), 1);
    assert!(alloc.usage_pct() > 0.0);
    assert!(!id.is_empty());
}

#[test]
fn allocate_exceeds_budget() {
    let budget = VramBudget {
        total_mb: 4096,
        allocated_mb: 0,
        session_budget_mb: 128,
    };
    let mut alloc = VramAllocator::new(budget);

    // Try to allocate 256 MB when budget is 128 MB.
    let result = alloc.allocate(AllocationPurpose::RenderTarget, 256 * 1024 * 1024);
    assert!(result.is_err());
}

#[test]
fn free_releases_vram() {
    let budget = VramBudget {
        total_mb: 4096,
        allocated_mb: 0,
        session_budget_mb: 256,
    };
    let mut alloc = VramAllocator::new(budget);

    let id = alloc
        .allocate(AllocationPurpose::ComputeBuffer, 32 * 1024 * 1024)
        .unwrap();

    assert_eq!(alloc.allocation_count(), 1);
    let before_available = alloc.available_bytes();

    assert!(alloc.free(&id));
    assert_eq!(alloc.allocation_count(), 0);
    assert!(alloc.available_bytes() > before_available);
}

#[test]
fn free_nonexistent_returns_false() {
    let mut alloc = VramAllocator::new(VramBudget::default());
    assert!(!alloc.free("does-not-exist"));
}

#[test]
fn available_bytes_correct() {
    let budget = VramBudget {
        total_mb: 4096,
        allocated_mb: 0,
        session_budget_mb: 256,
    };
    let alloc = VramAllocator::new(budget);

    assert_eq!(alloc.available_bytes(), 256 * 1024 * 1024);
}

#[test]
fn multiple_allocations() {
    let budget = VramBudget {
        total_mb: 4096,
        allocated_mb: 0,
        session_budget_mb: 256,
    };
    let mut alloc = VramAllocator::new(budget);

    let _id1 = alloc
        .allocate(AllocationPurpose::TextureAtlas, 64 * 1024 * 1024)
        .unwrap();
    let _id2 = alloc
        .allocate(AllocationPurpose::GlyphCache, 32 * 1024 * 1024)
        .unwrap();
    let _id3 = alloc
        .allocate(AllocationPurpose::StagingBuffer, 16 * 1024 * 1024)
        .unwrap();

    assert_eq!(alloc.allocation_count(), 3);
}

#[test]
fn usage_pct_zero_budget() {
    let budget = VramBudget {
        total_mb: 0,
        allocated_mb: 0,
        session_budget_mb: 0,
    };
    let alloc = VramAllocator::new(budget);
    assert_eq!(alloc.usage_pct(), 0.0);
}

#[test]
fn default_budget_is_256mb() {
    let budget = VramBudget::default();
    assert_eq!(budget.session_budget_mb, 256);
    assert_eq!(budget.allocated_mb, 0);
}

#[test]
fn allocation_purpose_display() {
    assert_eq!(AllocationPurpose::TextureAtlas.to_string(), "texture-atlas");
    assert_eq!(AllocationPurpose::RenderTarget.to_string(), "render-target");
    assert_eq!(
        AllocationPurpose::StagingBuffer.to_string(),
        "staging-buffer"
    );
    assert_eq!(
        AllocationPurpose::ComputeBuffer.to_string(),
        "compute-buffer"
    );
    assert_eq!(AllocationPurpose::GlyphCache.to_string(), "glyph-cache");
}
