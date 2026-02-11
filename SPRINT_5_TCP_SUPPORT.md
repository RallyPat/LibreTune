# Sprint 5 - TCP Connection Support Implementation

## Overview
Added support for TCP/IP connections to ECU simulators (or Wi-Fi bridges) in addition to existing Serial/USB connections. The implementation spans the full stack: Backend abstraction, Protocol refactoring, Middleware updates, and Frontend UI integration.

## Key Changes

### 1. Backend Abstraction (`libretune-core`)
- **New Module (`protocol/stream.rs`)**:
  - Defined `CommunicationChannel` trait to abstract over `SerialPort` and `TcpStream`.
  - Implemented `SerialChannel` wrapper for `serialport::SerialPort`.
  - Implemented `TcpChannel` wrapper for `std::net::TcpStream` with `peek`-based `bytes_to_read` optimization.
- **Protocol Refactoring (`protocol/connection.rs`)**:
  - Replaced hardcoded `Box<dyn SerialPort>` with `Box<dyn CommunicationChannel>`.
  - Updated `ConnectionConfig` to include `connection_type`, `tcp_host`, and `tcp_port`.
  - Updated `connect()` logic to handle TCP socket creation.
  - Refactored `read_exact_timeout`, `write_and_wait`, and other I/O helpers to use the generic channel.

### 2. Middleware (`libretune-app/src-tauri/src/lib.rs`)
- Updated `connect_to_ecu` command to accept optional TCP parameters:
  - `connection_type`: "Serial" or "Tcp" (case-insensitive)
  - `tcp_host`: Hostname or IP address (defaults to localhost)
  - `tcp_port`: Port number (defaults to 29001)
- Maps frontend parameters to backend `ConnectionConfig`.

### 3. Frontend UI (`libretune-app/src`)
- **State Management (`App.tsx`)**:
  - Added state for `connectionType`, `tcpHost`, and `tcpPort`.
  - Updated `connect()` function to pass these new parameters to the backend invoke call.
- **User Interface (`components/tuner-ui/Dialogs.tsx`)**:
  - Modified `ConnectionDialog` to include a "Connection Mode" toggle (Serial / TCP).
  - Added input fields for "Host Address" and "TCP Port" visible when TCP mode is selected.
  - Inputs are validated (number for port) and support default placeholders.

## Usage
1. Open the "Connection" dialog (Menu -> Connect).
2. Select "TCP / WiFi (Sim)" radio button.
3. Enter Host IP (e.g., `127.0.0.1`) and Port (e.g., `29001`).
4. Click Connect to establish a TCP session with the ECU simulator.

## Default Values
- **Host**: `127.0.0.1` (localhost)
- **Port**: `29001` (standard rusEFI console/simulator port)
