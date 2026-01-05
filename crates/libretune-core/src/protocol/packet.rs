//! Packet encoding/decoding
//!
//! Implements the binary packet format with CRC32 for the modern protocol.
//!
//! Packet format (rusEFI/msEnvelope_1.0):
//! - 2 bytes: Payload length (big-endian)
//! - N bytes: Payload
//! - 4 bytes: CRC32 (of payload only, NOT length+payload)

use byteorder::{BigEndian, ByteOrder};
use crc32fast::Hasher;

use super::{ProtocolError, MAX_PACKET_SIZE};

/// A protocol packet
#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet payload
    pub payload: Vec<u8>,
    /// CRC32 of the payload only
    pub crc: u32,
}

impl Packet {
    /// Create a new packet with the given payload
    pub fn new(payload: Vec<u8>) -> Self {
        let crc = calculate_crc(&payload);
        Self { payload, crc }
    }
    
    /// Decode a packet from raw bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProtocolError> {
        if data.len() < 6 {
            return Err(ProtocolError::InvalidResponse);
        }
        
        // Read length
        let length = BigEndian::read_u16(&data[0..2]) as usize;
        
        // Validate length
        if length > MAX_PACKET_SIZE {
            return Err(ProtocolError::BufferOverflow);
        }
        
        if data.len() < 2 + length + 4 {
            return Err(ProtocolError::InvalidResponse);
        }
        
        // Extract payload
        let payload = data[2..2+length].to_vec();
        
        // Read CRC
        let received_crc = BigEndian::read_u32(&data[2+length..2+length+4]);
        
        // Calculate expected CRC (of payload only - rusEFI format)
        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let expected_crc = hasher.finalize();
        
        // Verify CRC
        if received_crc != expected_crc {
            return Err(ProtocolError::CrcMismatch {
                expected: expected_crc,
                actual: received_crc,
            });
        }
        
        Ok(Self {
            payload,
            crc: received_crc,
        })
    }
    
    /// Encode the packet to raw bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(2 + self.payload.len() + 4);
        
        // Length (2 bytes, big-endian)
        let mut len_bytes = [0u8; 2];
        BigEndian::write_u16(&mut len_bytes, self.payload.len() as u16);
        bytes.extend_from_slice(&len_bytes);
        
        // Payload
        bytes.extend_from_slice(&self.payload);
        
        // Calculate CRC of payload only (rusEFI format)
        let mut hasher = Hasher::new();
        hasher.update(&self.payload);
        let crc = hasher.finalize();
        
        // CRC (4 bytes, big-endian)
        let mut crc_bytes = [0u8; 4];
        BigEndian::write_u32(&mut crc_bytes, crc);
        bytes.extend_from_slice(&crc_bytes);
        
        bytes
    }
    
    /// Get the total encoded size
    pub fn encoded_size(&self) -> usize {
        2 + self.payload.len() + 4
    }
}

/// Builder for constructing packets
pub struct PacketBuilder {
    payload: Vec<u8>,
}

impl PacketBuilder {
    /// Create a new packet builder
    pub fn new() -> Self {
        Self { payload: Vec::new() }
    }
    
    /// Add a command byte
    pub fn command(mut self, cmd: u8) -> Self {
        self.payload.push(cmd);
        self
    }
    
    /// Add a single byte
    pub fn byte(mut self, b: u8) -> Self {
        self.payload.push(b);
        self
    }
    
    /// Add a 16-bit value (big-endian)
    pub fn u16_be(mut self, value: u16) -> Self {
        let mut bytes = [0u8; 2];
        BigEndian::write_u16(&mut bytes, value);
        self.payload.extend_from_slice(&bytes);
        self
    }
    
    /// Add a 32-bit value (big-endian)
    pub fn u32_be(mut self, value: u32) -> Self {
        let mut bytes = [0u8; 4];
        BigEndian::write_u32(&mut bytes, value);
        self.payload.extend_from_slice(&bytes);
        self
    }
    
    /// Add raw bytes
    pub fn bytes(mut self, data: &[u8]) -> Self {
        self.payload.extend_from_slice(data);
        self
    }
    
    /// Build the packet
    pub fn build(self) -> Packet {
        Packet::new(self.payload)
    }
}

impl Default for PacketBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate CRC32 of the payload (for simple payloads without length prefix)
fn calculate_crc(payload: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    
    // Calculate length prefix
    let mut len_bytes = [0u8; 2];
    BigEndian::write_u16(&mut len_bytes, payload.len() as u16);
    
    hasher.update(&len_bytes);
    hasher.update(payload);
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_packet_roundtrip() {
        let original = Packet::new(vec![0x51]); // 'Q' command
        let encoded = original.to_bytes();
        let decoded = Packet::from_bytes(&encoded).expect("Should decode successfully");
        
        assert_eq!(original.payload, decoded.payload);
    }
    
    #[test]
    fn test_packet_builder() {
        let packet = PacketBuilder::new()
            .command(b'R')
            .byte(0) // Table
            .byte(0) // CAN ID
            .u16_be(0) // Offset
            .u16_be(128) // Length
            .build();
        
        assert_eq!(packet.payload[0], b'R');
        assert_eq!(packet.payload.len(), 7);
    }
    
    #[test]
    fn test_crc_verification() {
        let packet = Packet::new(vec![1, 2, 3, 4, 5]);
        let mut encoded = packet.to_bytes();
        
        // Corrupt a byte
        encoded[3] ^= 0xFF;
        
        // Should fail CRC check
        assert!(Packet::from_bytes(&encoded).is_err());
    }
}
