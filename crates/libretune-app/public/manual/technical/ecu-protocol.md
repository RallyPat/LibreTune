# ECU Communication Protocol

LibreTune supports multiple ECU communication protocols for different firmware families. This document explains the low-level protocol details, command structure, and error handling.

## Protocol Types

### Binary Protocol (Speeduino, MS2, MS3)

**Characteristics**:
- Fixed-length commands and responses
- Binary data packets
- CRC/checksum validation
- Synchronous request-response

### Text Protocol (rusEFI, FOME, epicEFI)

**Characteristics**:
- Newline-terminated commands
- ASCII text responses
- Line-based parsing
- Supports both interactive console and binary data modes

## Command Structure

### Page Read/Write (Binary)

**Page Read**:
```
Command:  'R' + page_number (1 byte)
Response: page_data (variable length, typically 512-1024 bytes)
```

**Page Write**:
```
Command:  'W' + page_number (1 byte) + offset (2 bytes) + data (N bytes)
Response: ACK (1 byte, typically 0x01)
```

**Example** (read page 1):
```
Send:    [0x52, 0x01]           # 'R', page 1
Receive: [0x50, 0x00, ...data...] # 512 bytes
```

### Memory Read/Write (MS2/MS3)

**Memory Read**:
```
Command: 'r' + offset (2 bytes BE) + length (2 bytes BE)
Response: data (length bytes)
```

**Example** (read 10 bytes from offset 0x1234):
```
Send:    [0x72, 0x12, 0x34, 0x00, 0x0A]
Receive: [0xAA, 0xBB, ...10 bytes...]
```

### Realtime Data Query

**Speeduino**:
```
Command: 'A'
Response: 75 bytes of realtime data
```

**rusEFI/FOME**:
```
Command: "r\x00\x34\x00\x4b" (read 75 bytes from offset 0x34)
Response: 75 bytes of realtime data
```

**MS2/MS3**:
```
Command: 'r' + 0x0000 + length (from INI ochBlockSize)
Response: ochBlockSize bytes
```

## Transport Layer

### Serial Port Configuration

**Settings**:
```rust
pub struct ConnectionSettings {
    pub port_name: String,       // e.g., "/dev/ttyUSB0", "COM3"
    pub baud_rate: u32,          // 9600, 115200, etc.
    pub data_bits: DataBits,     // Eight (default)
    pub parity: Parity,          // None (default)
    pub stop_bits: StopBits,     // One (default)
    pub flow_control: FlowControl, // None (default)
    pub timeout_ms: u64,         // Default: 1000ms
}
```

**Baud Rate Auto-Detection**:
LibreTune tries common rates in order:
1. 115200 (rusEFI/FOME default)
2. 57600
3. 38400
4. 19200
5. 9600 (Speeduino default)

### Timeout Handling

**Read Timeout** (inter-character):
```rust
// Read until no data for timeout_ms
let mut buffer = Vec::new();
loop {
    match port.read_with_timeout(&mut byte, timeout_ms) {
        Ok(1) => buffer.push(byte[0]),
        Ok(0) | Err(Timeout) => break,  // No more data
        Err(e) => return Err(e),
    }
}
```

**Write Timeout**:
```rust
// Write all bytes or fail
port.write_all(&command_bytes)
    .map_err(|e| ProtocolError::WriteTimeout(e))?;
```

## Error Detection

### CRC Validation (MS2/MS3)

**CRC-32**:
```rust
fn calculate_crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFFFFFF_u32;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}
```

**Packet Format**:
```
[command_byte, ...data..., crc32 (4 bytes LE)]
```

### Status Byte Validation (Speeduino)

**Status Byte Encoding**:
```
bit 0: Burn pending
bit 1: Write pending
bit 2-3: Reserved
bit 4-7: Error code
```

**Common Error Codes**:
- `0x00`: Success
- `0x10`: Invalid command
- `0x20`: Invalid page number
- `0x30`: CRC mismatch
- `0x40`: Buffer overflow
- `0x84`: Signature mismatch (132 decimal)

**Error Handling**:
```rust
let status = response[0];
let error_code = (status >> 4) & 0x0F;
match error_code {
    0x0 => Ok(()),
    0x8 => Err(ProtocolError::SignatureMismatch),
    _ => Err(ProtocolError::EcuError(error_code)),
}
```

## Data Synchronization

### Page Sync

**Full Page Read**:
```rust
pub fn sync_ecu_data(&mut self) -> Result<Vec<Vec<u8>>, ProtocolError> {
    let mut pages = Vec::new();
    for page_num in 0..self.num_pages {
        match self.read_page(page_num) {
            Ok(data) => pages.push(data),
            Err(e) => {
                log::warn!("Failed to read page {}: {}", page_num, e);
                pages.push(vec![0; self.page_sizes[page_num]]);
                // Continue syncing other pages
            }
        }
    }
    Ok(pages)
}
```

**Partial Sync** (on failure):
- Failed pages filled with zeros
- Status bar shows "⚠ Partial sync (X/Y pages)"
- User warned about incomplete data

### Write-Behind Caching

**Strategy**:
1. User edits → Write to local cache immediately (instant feedback)
2. Batch writes → Write to ECU in background (every 500ms or on manual save)
3. Verify → Read back after write and compare

**Cache Structure**:
```rust
pub struct TuneCache {
    pub pages: Vec<Vec<u8>>,         // Current state
    pub dirty_pages: HashSet<u8>,    // Pages needing ECU write
    pub write_queue: VecDeque<Write>, // Pending writes
}

pub struct Write {
    pub page: u8,
    pub offset: u16,
    pub data: Vec<u8>,
    pub timestamp: Instant,
}
```

## Console Protocol (rusEFI/FOME/epicEFI)

### Text-Based Commands

**Command Format**:
```
command\n
```

**Response Format**:
```
output line 1
output line 2
...
prompt>
```

**Example Session**:
```
> help\n
Available commands:
  help      - This help screen
  info      - ECU information
  set       - Set parameter
  get       - Get parameter
  ...
rusEFI>
```

### Binary Data Mode

rusEFI supports switching to binary mode for faster data transfer:

**Enter Binary Mode**:
```
Command: "binary\n"
Response: "OK\n"
```

**Binary Commands** (in binary mode):
```
[command_byte, ...args...]
```

**Exit Binary Mode**:
```
Command: [0xFF]
Response: (returns to text mode)
```

### FOME Fast Comms

FOME firmware supports an optimized protocol:

**Settings**:
```rust
pub struct Settings {
    pub fome_fast_comms_enabled: bool,  // Default: true
}
```

**Optimization**:
- Reduced command overhead
- Compressed response packets
- Automatic fallback to standard protocol on error

**Fallback Logic**:
```rust
if ecu_type == EcuType::FOME && settings.fome_fast_comms_enabled {
    match send_fast_command(cmd) {
        Ok(response) => return Ok(response),
        Err(e) => {
            log::warn!("Fast comms failed: {}, falling back to standard", e);
            // Fall through to standard protocol
        }
    }
}
send_standard_command(cmd)
```

## Connection State Machine

```
┌─────────────┐
│ Disconnected│
└──────┬──────┘
       │ connect()
       ↓
┌─────────────┐
│ Connecting  │──→ (port open, baud detect, handshake)
└──────┬──────┘
       │ success
       ↓
┌─────────────┐
│  Connected  │──→ (sync data, start realtime stream)
└──────┬──────┘
       │ disconnect() or error
       ↓
┌─────────────┐
│ Disconnected│
└─────────────┘
```

**State Transitions**:
```rust
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected { signature: String, ecu_type: EcuType },
    Error { message: String },
}
```

## Performance Metrics

**Recorded Metrics**:
```rust
pub struct ConnectionMetrics {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub errors: u64,
    pub avg_latency_ms: f64,
}
```

**Latency Measurement**:
```rust
let start = Instant::now();
send_command(cmd)?;
let response = read_response()?;
let latency = start.elapsed().as_millis();
metrics.update_latency(latency);
```

## Diagnostic Tools

### Tooth Logger

**Command** (Speeduino):
```
Send: 'H'
Response: 512 bytes (256 tooth times, 2 bytes each)
```

**Data Structure**:
```rust
pub struct ToothLogEntry {
    pub tooth_number: u16,
    pub tooth_time_us: u16,  // Microseconds since last tooth
    pub crank_angle: f32,    // Calculated
}
```

### Composite Logger

**Command** (Speeduino):
```
Send: 'J' (primary trigger)
      'O' (secondary trigger)
      'X' (sync/cam trigger)
Response: Variable length, timestamps + logic levels
```

**Data Structure**:
```rust
pub struct CompositeLogEntry {
    pub time_us: u64,
    pub primary: bool,
    pub secondary: bool,
    pub sync: bool,
    pub voltage: Option<f32>,
}
```

## Security Considerations

### Buffer Overflow Protection

**Bounded Reads**:
```rust
let mut buffer = vec![0u8; MAX_PACKET_SIZE];
let bytes_read = port.read(&mut buffer)?;
if bytes_read > MAX_PACKET_SIZE {
    return Err(ProtocolError::BufferOverflow);
}
buffer.truncate(bytes_read);
```

### Command Validation

**Whitelist Approach**:
```rust
const ALLOWED_COMMANDS: &[u8] = b"RWArwABCEGHJOQSXZ";

fn validate_command(cmd: u8) -> Result<(), ProtocolError> {
    if ALLOWED_COMMANDS.contains(&cmd) {
        Ok(())
    } else {
        Err(ProtocolError::InvalidCommand(cmd))
    }
}
```

## Source Code Reference

- Protocol implementation: `crates/libretune-core/src/protocol/`
  - `mod.rs` - Protocol trait and common types
  - `connection.rs` - Connection management
  - `commands.rs` - Command/response structures
  - `serial.rs` - Serial port abstraction
- Tauri integration: `crates/libretune-app/src-tauri/src/lib.rs` (connect, sync, console commands)
- Console UI: `crates/libretune-app/src/components/console/EcuConsole.tsx`

## See Also

- [Connecting to Your ECU](../getting-started/connecting.md) - User guide for connection setup
- [Supported ECUs](../reference/supported-ecus.md) - Platform-specific protocol details
- [Diagnostic Loggers](../features/diagnostic-loggers.md) - Using tooth/composite loggers
