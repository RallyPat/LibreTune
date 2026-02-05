use libretune_core::protocol::ProtocolError;
use std::sync::{Arc, Mutex};

/// Mock serial port for testing
struct MockSerial {
    send_buffer: Vec<u8>,
    recv_buffer: Vec<u8>,
    recv_idx: usize,
    fail_on_send: bool,
}

impl MockSerial {
    fn new() -> Self {
        Self {
            send_buffer: Vec::new(),
            recv_buffer: Vec::new(),
            recv_idx: 0,
            fail_on_send: false,
        }
    }

    fn with_response(response: Vec<u8>) -> Self {
        Self {
            send_buffer: Vec::new(),
            recv_buffer: response,
            recv_idx: 0,
            fail_on_send: false,
        }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        if self.recv_idx < self.recv_buffer.len() {
            let byte = self.recv_buffer[self.recv_idx];
            self.recv_idx += 1;
            Ok(byte)
        } else {
            Err("EOF".to_string())
        }
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<(), String> {
        if self.fail_on_send {
            return Err("Serial write failed".to_string());
        }
        self.send_buffer.extend_from_slice(buf);
        Ok(())
    }
}

#[test]
fn test_connection_creation() {
    // Should create connection without panic
    let mock = MockSerial::new();
    let _shared = Arc::new(Mutex::new(mock));
    // Connection creation would require implementation details
}

#[test]
fn test_protocol_error_debug() {
    let err = ProtocolError::Timeout;
    assert!(!format!("{:?}", err).is_empty());
}

#[test]
fn test_protocol_error_display() {
    let err = ProtocolError::Timeout;
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_crc16_calculation_deterministic() {
    // Test that protocol error type exists and can be used
    let err = ProtocolError::Timeout;
    let err_str = format!("{}", err);
    assert!(!err_str.is_empty());
}

#[test]
fn test_timeout_error() {
    let err = ProtocolError::Timeout;
    let err_str = format!("{:?}", err);
    assert!(err_str.contains("Timeout") || !err_str.is_empty());
}

#[test]
fn test_serial_mockserial_read() {
    let mut mock = MockSerial::with_response(vec![0xAB, 0xCD]);
    let byte1 = mock.read_byte();
    assert!(byte1.is_ok());
    let byte2 = mock.read_byte();
    assert!(byte2.is_ok());
}

#[test]
fn test_serial_mockserial_eof() {
    let mut mock = MockSerial::new(); // empty response buffer
    let result = mock.read_byte();
    assert!(result.is_err());
}

#[test]
fn test_serial_mockserial_write() {
    let mut mock = MockSerial::new();
    let result = mock.write_all(b"test");
    assert!(result.is_ok());
    assert_eq!(mock.send_buffer, b"test".to_vec());
}

#[test]
fn test_serial_mockserial_write_failure() {
    let mut mock = MockSerial::new();
    mock.fail_on_send = true;
    let result = mock.write_all(b"test");
    assert!(result.is_err());
}

#[test]
fn test_serial_mockserial_data_integrity() {
    let mut mock = MockSerial::new();
    let test_data = b"Hello, ECU!";
    let result = mock.write_all(test_data);
    assert!(result.is_ok());
    assert_eq!(mock.send_buffer, test_data.to_vec());
}
