//! Command Builder
//!
//! Builds protocol commands from INI format strings.
//!
//! Format string placeholders:
//! - %2i : 16-bit page index (big-endian)
//! - %2o : 16-bit offset (big-endian)  
//! - %2c : 16-bit count/length (big-endian)
//! - %v  : variable-length data bytes
//!
//! For rusEFI/modern protocol, commands are wrapped in CRC framing.

use super::{Packet, ProtocolError};
use byteorder::{BigEndian, ByteOrder, LittleEndian};

/// Build a command from an INI format string
pub struct CommandBuilder {
    /// Use little-endian byte order (for rusEFI)
    little_endian: bool,
}

impl CommandBuilder {
    pub fn new(little_endian: bool) -> Self {
        Self { little_endian }
    }

    /// Build a read command from format string
    /// e.g., "R%2i%2o%2c" with page=0, offset=0, count=256
    pub fn build_read_command(
        &self,
        format: &str,
        page: u16,
        offset: u16,
        count: u16,
    ) -> Result<Vec<u8>, ProtocolError> {
        self.build_command(format, page, offset, count, &[])
    }

    /// Build a write command from format string
    /// e.g., "C%2i%2o%2c%v" with page=0, offset=0, count=len(data), data
    pub fn build_write_command(
        &self,
        format: &str,
        page: u16,
        offset: u16,
        data: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        self.build_command(format, page, offset, data.len() as u16, data)
    }

    /// Build a burn command from format string
    /// e.g., "B%2i" with page=0
    pub fn build_burn_command(&self, format: &str, page: u16) -> Result<Vec<u8>, ProtocolError> {
        self.build_command(format, page, 0, 0, &[])
    }

    /// Build a CRC check command from format string
    /// e.g., "k%2i%2o%2c" with page=0, offset=0, count=pageSize
    pub fn build_crc_command(
        &self,
        format: &str,
        page: u16,
        offset: u16,
        count: u16,
    ) -> Result<Vec<u8>, ProtocolError> {
        self.build_command(format, page, offset, count, &[])
    }

    /// Generic command builder that parses format string and substitutes values
    fn build_command(
        &self,
        format: &str,
        page: u16,
        offset: u16,
        count: u16,
        data: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut result = Vec::new();
        let chars: Vec<char> = format.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '%' && i + 2 <= chars.len() {
                // Check for format specifier
                if i + 2 < chars.len() && chars[i + 1] == '2' {
                    match chars[i + 2] {
                        'i' => {
                            // 16-bit page index
                            let mut buf = [0u8; 2];
                            if self.little_endian {
                                LittleEndian::write_u16(&mut buf, page);
                            } else {
                                BigEndian::write_u16(&mut buf, page);
                            }
                            result.extend_from_slice(&buf);
                            i += 3;
                            continue;
                        }
                        'o' => {
                            // 16-bit offset
                            let mut buf = [0u8; 2];
                            if self.little_endian {
                                LittleEndian::write_u16(&mut buf, offset);
                            } else {
                                BigEndian::write_u16(&mut buf, offset);
                            }
                            result.extend_from_slice(&buf);
                            i += 3;
                            continue;
                        }
                        'c' => {
                            // 16-bit count
                            let mut buf = [0u8; 2];
                            if self.little_endian {
                                LittleEndian::write_u16(&mut buf, count);
                            } else {
                                BigEndian::write_u16(&mut buf, count);
                            }
                            result.extend_from_slice(&buf);
                            i += 3;
                            continue;
                        }
                        _ => {}
                    }
                } else if i + 1 < chars.len() && chars[i + 1] == 'v' {
                    // Variable data
                    result.extend_from_slice(data);
                    i += 2;
                    continue;
                }
            }

            // Regular character - add as byte
            result.push(chars[i] as u8);
            i += 1;
        }

        Ok(result)
    }

    /// Wrap command in CRC packet for modern protocol
    pub fn wrap_in_packet(&self, command: Vec<u8>) -> Packet {
        Packet::new(command)
    }
}

/// Helper to build a simple single-byte command (legacy protocol)
pub fn legacy_command(cmd: char) -> Vec<u8> {
    vec![cmd as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_command_big_endian() {
        let builder = CommandBuilder::new(false);
        let cmd = builder
            .build_read_command("R%2i%2o%2c", 0, 100, 256)
            .unwrap();
        assert_eq!(
            cmd,
            vec![
                b'R', 0, 0, // page 0
                0, 100, // offset 100
                1, 0, // count 256
            ]
        );
    }

    #[test]
    fn test_read_command_little_endian() {
        let builder = CommandBuilder::new(true);
        let cmd = builder
            .build_read_command("R%2i%2o%2c", 1, 0x0100, 0x0200)
            .unwrap();
        assert_eq!(
            cmd,
            vec![
                b'R', 1, 0, // page 1 (little-endian)
                0, 1, // offset 0x0100 (little-endian)
                0, 2, // count 0x0200 (little-endian)
            ]
        );
    }

    #[test]
    fn test_write_command() {
        let builder = CommandBuilder::new(false);
        let data = vec![0xAA, 0xBB, 0xCC];
        let cmd = builder
            .build_write_command("C%2i%2o%2c%v", 0, 50, &data)
            .unwrap();
        assert_eq!(
            cmd,
            vec![
                b'C', 0, 0, // page 0
                0, 50, // offset 50
                0, 3, // count 3
                0xAA, 0xBB, 0xCC, // data
            ]
        );
    }

    #[test]
    fn test_burn_command() {
        let builder = CommandBuilder::new(false);
        let cmd = builder.build_burn_command("B%2i", 0).unwrap();
        assert_eq!(cmd, vec![b'B', 0, 0]);
    }

    #[test]
    fn test_empty_format() {
        let builder = CommandBuilder::new(false);
        // Empty burn command for non-burnable pages
        let cmd = builder.build_burn_command("", 0).unwrap();
        assert!(cmd.is_empty());
    }
}
