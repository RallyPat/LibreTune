//! ECU memory management
//!
//! Provides a high-level interface to ECU memory pages.

use crate::ini::EcuDefinition;
use std::collections::HashMap;

/// Manages the ECU's memory state
#[derive(Default)]
pub struct EcuMemory {
    /// Raw page data
    pages: HashMap<u8, Vec<u8>>,
    /// Page sizes from definition
    page_sizes: Vec<u16>,
    /// Number of pages
    n_pages: u8,
}

impl EcuMemory {
    /// Create a new ECU memory model from a definition
    pub fn from_definition(definition: &EcuDefinition) -> Self {
        let mut pages = HashMap::new();

        for (i, size) in definition.page_sizes.iter().enumerate() {
            pages.insert(i as u8, vec![0u8; *size as usize]);
        }

        Self {
            pages,
            page_sizes: definition.page_sizes.clone(),
            n_pages: definition.n_pages,
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

    /// Read raw bytes from a page
    pub fn read_bytes(&self, page: u8, offset: u16, length: u16) -> Option<&[u8]> {
        let page_data = self.pages.get(&page)?;
        let start = offset as usize;
        let end = start + length as usize;

        if end <= page_data.len() {
            Some(&page_data[start..end])
        } else {
            None
        }
    }

    /// Write raw bytes to a page
    pub fn write_bytes(&mut self, page: u8, offset: u16, data: &[u8]) -> bool {
        if let Some(page_data) = self.pages.get_mut(&page) {
            let start = offset as usize;
            let end = start + data.len();

            if end <= page_data.len() {
                page_data[start..end].copy_from_slice(data);
                return true;
            }
        }
        false
    }

    /// Load a complete page from data
    pub fn load_page(&mut self, page: u8, data: Vec<u8>) {
        self.pages.insert(page, data);
    }

    /// Get a complete page
    pub fn get_page(&self, page: u8) -> Option<&[u8]> {
        self.pages.get(&page).map(|v| v.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_read_write() {
        let mut memory = EcuMemory::default();
        memory.load_page(0, vec![0u8; 256]);

        assert!(memory.write_bytes(0, 10, &[1, 2, 3, 4]));

        let data = memory.read_bytes(0, 10, 4).unwrap();
        assert_eq!(data, &[1, 2, 3, 4]);
    }
}
