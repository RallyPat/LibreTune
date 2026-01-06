//! Shadow memory for tracking changes
//!
//! Tracks modifications to ECU memory for efficient synchronization.

use std::collections::HashSet;

/// Tracks changes to ECU memory
pub struct ShadowMemory {
    /// Set of (page, offset) pairs that have been modified
    dirty: HashSet<(u8, u16)>,
    #[allow(dead_code)]
    original: Vec<u8>,
}

impl ShadowMemory {
    /// Create a new shadow memory tracker
    pub fn new() -> Self {
        Self {
            dirty: HashSet::new(),
            original: Vec::new(),
        }
    }

    /// Mark a range as dirty
    pub fn mark_dirty(&mut self, page: u8, offset: u16, length: u16) {
        for i in 0..length {
            self.dirty.insert((page, offset + i));
        }
    }

    /// Check if a position is dirty
    pub fn is_dirty(&self, page: u8, offset: u16) -> bool {
        self.dirty.contains(&(page, offset))
    }

    /// Check if any changes are pending
    pub fn has_changes(&self) -> bool {
        !self.dirty.is_empty()
    }

    /// Get all dirty pages
    pub fn dirty_pages(&self) -> Vec<u8> {
        let mut pages: Vec<u8> = self.dirty.iter().map(|(p, _)| *p).collect();
        pages.sort();
        pages.dedup();
        pages
    }

    /// Clear all dirty flags
    pub fn clear(&mut self) {
        self.dirty.clear();
    }

    /// Count of dirty bytes
    pub fn dirty_count(&self) -> usize {
        self.dirty.len()
    }
}

impl Default for ShadowMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_tracking() {
        let mut shadow = ShadowMemory::new();

        assert!(!shadow.has_changes());

        shadow.mark_dirty(0, 10, 4);

        assert!(shadow.has_changes());
        assert!(shadow.is_dirty(0, 10));
        assert!(shadow.is_dirty(0, 13));
        assert!(!shadow.is_dirty(0, 14));

        shadow.clear();
        assert!(!shadow.has_changes());
    }
}
