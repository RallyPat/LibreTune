use serialport::SerialPort;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Abstraction for communication channels (Serial or TCP)
pub trait CommunicationChannel: Read + Write + Send {
    /// Set timeout for read/write operations
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()>;

    /// Clear input buffers
    fn clear_input_buffer(&mut self) -> io::Result<()>;

    /// Clear output buffers
    fn clear_output_buffer(&mut self) -> io::Result<()>;

    /// Try to clone the channel
    fn try_clone(&self) -> io::Result<Box<dyn CommunicationChannel>>;

    /// Get number of bytes available to read
    fn bytes_to_read(&mut self) -> io::Result<u32>;
}

/// Serial port wrapper implementing CommunicationChannel
pub struct SerialChannel {
    port: Box<dyn SerialPort>,
}

impl SerialChannel {
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        Self { port }
    }
}

impl Read for SerialChannel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.port.read(buf)
    }
}

impl Write for SerialChannel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.port.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.port.flush()
    }
}

impl CommunicationChannel for SerialChannel {
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.port
            .set_timeout(timeout)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn clear_input_buffer(&mut self) -> io::Result<()> {
        self.port
            .clear(serialport::ClearBuffer::Input)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn clear_output_buffer(&mut self) -> io::Result<()> {
        self.port
            .clear(serialport::ClearBuffer::Output)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }

    fn try_clone(&self) -> io::Result<Box<dyn CommunicationChannel>> {
        let port_clone = self
            .port
            .try_clone()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Box::new(SerialChannel::new(port_clone)))
    }

    fn bytes_to_read(&mut self) -> io::Result<u32> {
        self.port
            .bytes_to_read()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
    }
}

/// TCP stream wrapper implementing CommunicationChannel
pub struct TcpChannel {
    stream: TcpStream,
}

impl TcpChannel {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }
}

impl Read for TcpChannel {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf)
    }
}

impl Write for TcpChannel {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl CommunicationChannel for TcpChannel {
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.stream.set_read_timeout(Some(timeout))?;
        self.stream.set_write_timeout(Some(timeout))?;
        Ok(())
    }

    fn clear_input_buffer(&mut self) -> io::Result<()> {
        // TCP doesn't have a direct "clear buffer" syscall.
        // We can set a very short timeout and read until empty.
        // Or simply do nothing and rely on the protocol to handle unexpected bytes.
        // For now, let's try to drain it with a non-blocking read.

        // This is non-trivial on generic streams.
        // A simple approach is to rely on protocol state to ignore garbage.
        // However, let's try to set non-blocking, read until WouldBlock, then restore.

        self.stream.set_nonblocking(true)?;
        let mut buf = [0u8; 1024];
        loop {
            match self.stream.read(&mut buf) {
                Ok(0) => break,                                           // EOF
                Ok(_) => continue,                                        // Discard
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break, // Empty
                Err(e) => {
                    // Restore blocking and return error
                    // Ignore error on restoring non-blocking for now as we are returning an error anyway
                    let _ = self.stream.set_nonblocking(false);
                    return Err(e);
                }
            }
        }
        self.stream.set_nonblocking(false)?;
        Ok(())
    }

    fn clear_output_buffer(&mut self) -> io::Result<()> {
        // TCP output buffer is managed by OS. Flush calls it.
        self.stream.flush()
    }

    fn try_clone(&self) -> io::Result<Box<dyn CommunicationChannel>> {
        let stream_clone = self.stream.try_clone()?;
        Ok(Box::new(TcpChannel::new(stream_clone)))
    }

    fn bytes_to_read(&mut self) -> io::Result<u32> {
        self.stream.set_nonblocking(true)?;
        // Use a reasonably large buffer to detect available bytes.
        // peek() returns minimal(available, buffer_size).
        let mut buf = [0u8; 8192];
        let result = self.stream.peek(&mut buf);
        self.stream.set_nonblocking(false)?;

        match result {
            Ok(n) => Ok(n as u32),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
            Err(e) => Err(e),
        }
    }
}
