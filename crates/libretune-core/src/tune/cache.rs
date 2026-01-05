//! Tune Cache - Local data buffer for ECU tuning
//!
//! The TuneCache holds a local copy of ECU memory, enabling:
//! - Offline editing without ECU connection
//! - Dirty tracking for modified values
//! - Batch writes to minimize ECU communication
//! - Loading state tracking per page

use std::collections::HashMap;
use crate::ecu::ShadowMemory;
use crate::ini::EcuDefinition;

/// Loading state for a memory page
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageState {
    /// Page has not been loaded yet
    NotLoaded,
    /// Page is currently being loaded from ECU
    Loading,
    /// Page data is current and matches ECU
    Clean,
    /// Page has local modifications not yet sent to ECU
    Dirty,
    /// Page has been sent to ECU but not burned to flash
    Pending,
    /// Failed to load page
    Error,
}

/// Holds local copy of ECU data with change tracking
pub struct TuneCache {
    /// Raw page data
    pages: HashMap<u8, Vec<u8>>,
    /// Page sizes from definition
    page_sizes: Vec<u16>,
    /// Number of pages
    n_pages: u8,
    /// State of each page
    page_states: HashMap<u8, PageState>,
    /// Dirty byte tracking
    shadow: ShadowMemory,
    /// Whether we have any pending burns
    has_pending_burn: bool,
    /// Local values for PC variables (not stored on ECU)
    pub local_values: HashMap<String, f64>,
}

impl TuneCache {
    /// Create a new tune cache from ECU definition
    pub fn from_definition(definition: &EcuDefinition) -> Self {
        let mut pages = HashMap::new();
        let mut page_states = HashMap::new();
        
        for (i, size) in definition.page_sizes.iter().enumerate() {
            pages.insert(i as u8, vec![0u8; *size as usize]);
            page_states.insert(i as u8, PageState::NotLoaded);
        }
        
        Self {
            pages,
            page_sizes: definition.page_sizes.clone(),
            n_pages: definition.n_pages,
            page_states,
            shadow: ShadowMemory::new(),
            has_pending_burn: false,
            local_values: HashMap::new(),
        }
    }
    
    /// Get the number of pages
    pub fn page_count(&self) -> u8 {
        self.n_pages
    }
    
    /// Get the size of a page
    pub fn page_size(&self, page: u8) -> Option<u16> {
        self.page_sizes.get(page as usize).copied()
    }
    
    /// Get the state of a page
    pub fn page_state(&self, page: u8) -> PageState {
        *self.page_states.get(&page).unwrap_or(&PageState::NotLoaded)
    }
    
    /// Check if all pages are loaded
    pub fn is_fully_loaded(&self) -> bool {
        for page in 0..self.n_pages {
            match self.page_state(page) {
                PageState::Clean | PageState::Dirty | PageState::Pending => continue,
                _ => return false,
            }
        }
        true
    }
    
    /// Check if any page is currently loading
    pub fn is_loading(&self) -> bool {
        self.page_states.values().any(|s| *s == PageState::Loading)
    }
    
    /// Get list of pages that need to be loaded
    pub fn pages_to_load(&self) -> Vec<u8> {
        (0..self.n_pages)
            .filter(|p| self.page_state(*p) == PageState::NotLoaded)
            .collect()
    }
    
    /// Mark a page as loading
    pub fn mark_loading(&mut self, page: u8) {
        self.page_states.insert(page, PageState::Loading);
    }
    
    /// Mark a page as failed to load
    pub fn mark_error(&mut self, page: u8) {
        self.page_states.insert(page, PageState::Error);
    }
    
    /// Load page data from ECU response
    pub fn load_page(&mut self, page: u8, data: Vec<u8>) {
        self.pages.insert(page, data);
        self.page_states.insert(page, PageState::Clean);
    }
    
    /// Read raw bytes from a page (returns None if page not loaded)
    pub fn read_bytes(&self, page: u8, offset: u16, length: u16) -> Option<&[u8]> {
        // Check page is loaded
        match self.page_state(page) {
            PageState::Clean | PageState::Dirty | PageState::Pending => {}
            _ => return None,
        }
        
        let page_data = self.pages.get(&page)?;
        let start = offset as usize;
        let end = start + length as usize;
        
        if end <= page_data.len() {
            Some(&page_data[start..end])
        } else {
            None
        }
    }
    
    /// Write raw bytes to a page (marks as dirty)
    pub fn write_bytes(&mut self, page: u8, offset: u16, data: &[u8]) -> bool {
        let start = offset as usize;
        let end = start + data.len();
        
        // Get page size from definition before mutable borrow
        let default_page_size = self.page_size(page).unwrap_or(0) as usize;
        
        // Get or create the page
        let page_data = self.pages.entry(page).or_insert_with(|| {
            // If page doesn't exist, create it with size from definition, or expand to fit the write
            let min_size = end.max(default_page_size);
            vec![0u8; min_size]
        });
        
        // Expand page if needed
        if end > page_data.len() {
            let new_size = end.max(default_page_size);
            page_data.resize(new_size, 0);
        }
        
        // Write the data
        page_data[start..end].copy_from_slice(data);
        self.shadow.mark_dirty(page, offset, data.len() as u16);
        self.page_states.insert(page, PageState::Dirty);
        true
    }
    
    /// Get a complete page
    pub fn get_page(&self, page: u8) -> Option<&[u8]> {
        self.pages.get(&page).map(|v| v.as_slice())
    }
    
    /// Check if there are any local modifications
    pub fn has_dirty_data(&self) -> bool {
        self.shadow.has_changes()
    }
    
    /// Check if there are pending burns (sent to ECU but not burned)
    pub fn has_pending_burn(&self) -> bool {
        self.has_pending_burn
    }
    
    /// Get count of dirty bytes
    pub fn dirty_byte_count(&self) -> usize {
        self.shadow.dirty_count()
    }
    
    /// Get pages with dirty data
    pub fn dirty_pages(&self) -> Vec<u8> {
        self.shadow.dirty_pages()
    }
    
    /// Mark pages as pending (sent to ECU but not burned)
    pub fn mark_pending(&mut self) {
        for page in self.shadow.dirty_pages() {
            self.page_states.insert(page, PageState::Pending);
        }
        self.shadow.clear();
        self.has_pending_burn = true;
    }
    
    /// Mark burn as complete
    pub fn mark_burned(&mut self) {
        for (_, state) in self.page_states.iter_mut() {
            if *state == PageState::Pending {
                *state = PageState::Clean;
            }
        }
        self.has_pending_burn = false;
    }
    
    /// Revert to clean state (discard changes)
    pub fn revert(&mut self) {
        self.shadow.clear();
        for (_, state) in self.page_states.iter_mut() {
            if *state == PageState::Dirty {
                *state = PageState::NotLoaded; // Will need to reload
            }
        }
    }
    
    /// Get dirty ranges for a page (for efficient writes)
    /// Returns list of (offset, length) pairs
    pub fn dirty_ranges(&self, page: u8) -> Vec<(u16, u16)> {
        let mut ranges = Vec::new();
        let mut start: Option<u16> = None;
        let mut length: u16 = 0;
        
        let page_size = self.page_size(page).unwrap_or(0);
        
        for offset in 0..page_size {
            if self.shadow.is_dirty(page, offset) {
                match start {
                    None => {
                        start = Some(offset);
                        length = 1;
                    }
                    Some(_) => {
                        length += 1;
                    }
                }
            } else if let Some(s) = start {
                ranges.push((s, length));
                start = None;
                length = 0;
            }
        }
        
        // Don't forget trailing range
        if let Some(s) = start {
            ranges.push((s, length));
        }
        
        ranges
    }
}

impl Default for TuneCache {
    fn default() -> Self {
        Self {
            pages: HashMap::new(),
            page_sizes: Vec::new(),
            n_pages: 0,
            page_states: HashMap::new(),
            shadow: ShadowMemory::new(),
            has_pending_burn: false,
            local_values: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_cache() -> TuneCache {
        let mut cache = TuneCache::default();
        cache.n_pages = 2;
        cache.page_sizes = vec![256, 512];
        cache.pages.insert(0, vec![0u8; 256]);
        cache.pages.insert(1, vec![0u8; 512]);
        cache.page_states.insert(0, PageState::Clean);
        cache.page_states.insert(1, PageState::Clean);
        cache
    }
    
    #[test]
    fn test_read_write() {
        let mut cache = create_test_cache();
        
        assert!(!cache.has_dirty_data());
        
        // Write some data
        assert!(cache.write_bytes(0, 10, &[1, 2, 3, 4]));
        
        // Should be dirty now
        assert!(cache.has_dirty_data());
        assert_eq!(cache.page_state(0), PageState::Dirty);
        
        // Read back
        let data = cache.read_bytes(0, 10, 4).unwrap();
        assert_eq!(data, &[1, 2, 3, 4]);
    }
    
    #[test]
    fn test_dirty_ranges() {
        let mut cache = create_test_cache();
        
        // Write two non-contiguous ranges
        cache.write_bytes(0, 10, &[1, 2, 3]);
        cache.write_bytes(0, 20, &[4, 5]);
        
        let ranges = cache.dirty_ranges(0);
        assert_eq!(ranges, vec![(10, 3), (20, 2)]);
    }
    
    #[test]
    fn test_loading_state() {
        let mut cache = TuneCache::default();
        cache.n_pages = 2;
        cache.page_sizes = vec![256, 512];
        cache.pages.insert(0, vec![0u8; 256]);
        cache.pages.insert(1, vec![0u8; 512]);
        cache.page_states.insert(0, PageState::NotLoaded);
        cache.page_states.insert(1, PageState::NotLoaded);
        
        assert!(!cache.is_fully_loaded());
        assert_eq!(cache.pages_to_load(), vec![0, 1]);
        
        cache.load_page(0, vec![0u8; 256]);
        assert!(!cache.is_fully_loaded());
        assert_eq!(cache.pages_to_load(), vec![1]);
        
        cache.load_page(1, vec![0u8; 512]);
        assert!(cache.is_fully_loaded());
        assert!(cache.pages_to_load().is_empty());
    }
    
    #[test]
    fn test_pending_burn() {
        let mut cache = create_test_cache();
        
        cache.write_bytes(0, 10, &[1, 2, 3]);
        assert!(cache.has_dirty_data());
        assert!(!cache.has_pending_burn());
        
        cache.mark_pending();
        assert!(!cache.has_dirty_data());
        assert!(cache.has_pending_burn());
        assert_eq!(cache.page_state(0), PageState::Pending);
        
        cache.mark_burned();
        assert!(!cache.has_pending_burn());
        assert_eq!(cache.page_state(0), PageState::Clean);
    }
}
