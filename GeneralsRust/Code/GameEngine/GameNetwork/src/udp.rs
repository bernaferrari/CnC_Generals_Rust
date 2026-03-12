//! UDP socket wrapper matching the legacy C++ UDP class.

use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, ToSocketAddrs, UdpSocket};

#[cfg(unix)]
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SockStat {
    Ok = 0,
    Unknown = -1,
    IsConn = -2,
    InProgress = -3,
    Already = -4,
    Again = -5,
    AddrInUse = -6,
    AddrNotAvail = -7,
    BadF = -8,
    ConnRefused = -9,
    Intr = -10,
    NotSock = -11,
    Pipe = -12,
    WouldBlock = -13,
    Inval = -14,
    TimedOut = -15,
}

fn map_error(err: &io::Error) -> SockStat {
    match err.kind() {
        io::ErrorKind::WouldBlock => SockStat::WouldBlock,
        io::ErrorKind::Interrupted => SockStat::Intr,
        io::ErrorKind::TimedOut => SockStat::TimedOut,
        io::ErrorKind::AlreadyExists => SockStat::AddrInUse,
        io::ErrorKind::AddrInUse => SockStat::AddrInUse,
        io::ErrorKind::AddrNotAvailable => SockStat::AddrNotAvail,
        io::ErrorKind::ConnectionRefused => SockStat::ConnRefused,
        io::ErrorKind::InvalidInput => SockStat::Inval,
        io::ErrorKind::NotConnected => SockStat::NotSock,
        _ => SockStat::Unknown,
    }
}

/// C++-style UDP wrapper.
pub struct Udp {
    socket: Option<UdpSocket>,
    my_ip: u32,
    my_port: u16,
    last_error: Option<io::Error>,
}

impl Default for Udp {
    fn default() -> Self {
        Self::new()
    }
}

impl Udp {
    pub fn new() -> Self {
        Self {
            socket: None,
            my_ip: 0,
            my_port: 0,
            last_error: None,
        }
    }

    pub fn bind_host(&mut self, host: &str, port: u16) -> SockStat {
        if let Ok(ip) = host.parse::<Ipv4Addr>() {
            return self.bind_ip(u32::from(ip).to_be(), port);
        }

        let resolved = (host, port)
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.find(|addr| matches!(addr, std::net::SocketAddr::V4(_))));

        if let Some(std::net::SocketAddr::V4(addr)) = resolved {
            self.bind_ip(u32::from(*addr.ip()).to_be(), port)
        } else {
            SockStat::Unknown
        }
    }

    pub fn bind_ip(&mut self, ip_host_order: u32, port: u16) -> SockStat {
        let ip = Ipv4Addr::from(ip_host_order.to_be());
        let addr = SocketAddrV4::new(ip, port);
        match UdpSocket::bind(addr) {
            Ok(socket) => {
                if socket.set_nonblocking(true).is_err() {
                    // Keep socket even if non-blocking fails.
                }
                self.socket = Some(socket);
                self.my_ip = ip_host_order;
                self.my_port = port;
                self.last_error = None;
                SockStat::Ok
            }
            Err(err) => {
                self.last_error = Some(err);
                SockStat::Unknown
            }
        }
    }

    pub fn get_local_addr(&self) -> (u32, u16) {
        (self.my_ip, self.my_port)
    }

    /// Get the raw file descriptor (Unix only)
    #[cfg(unix)]
    pub fn get_fd(&self) -> i32 {
        self.socket.as_ref().map_or(-1, |s| s.as_raw_fd())
    }

    #[cfg(not(unix))]
    pub fn get_fd(&self) -> i32 {
        -1
    }

    pub fn write(&mut self, msg: &[u8], ip_host_order: u32, port: u16) -> i32 {
        if ip_host_order == 0 || port == 0 {
            return SockStat::AddrNotAvail as i32;
        }

        self.clear_status();

        let socket = match &self.socket {
            Some(socket) => socket,
            None => return SockStat::NotSock as i32,
        };

        let ip = Ipv4Addr::from(ip_host_order.to_be());
        let addr = SocketAddrV4::new(ip, port);
        match socket.send_to(msg, addr) {
            Ok(count) => count as i32,
            Err(err) => {
                self.last_error = Some(err);
                -1
            }
        }
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> i32 {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return SockStat::NotSock as i32,
        };

        match socket.recv_from(buffer) {
            Ok((count, _addr)) => count as i32,
            Err(err) => {
                if err.kind() == io::ErrorKind::WouldBlock {
                    0
                } else {
                    self.last_error = Some(err);
                    -1
                }
            }
        }
    }

    pub fn clear_status(&mut self) {
        self.last_error = None;
    }

    pub fn status(&self) -> SockStat {
        match &self.last_error {
            None => SockStat::Ok,
            Some(err) => map_error(err),
        }
    }

    /// Set socket receive buffer size using socket2 (cross-platform, safe).
    /// Returns false if socket is not bound.
    #[cfg(unix)]
    pub fn set_input_buffer(&self, bytes: usize) -> bool {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return false,
        };
        let fd = socket.as_raw_fd();
        // SAFETY: We're wrapping an existing valid file descriptor
        let sock2 = unsafe { socket2::Socket::from_raw_fd(fd) };
        let result = sock2.set_recv_buffer_size(bytes);
        // Prevent closing the fd when sock2 is dropped
        sock2.into_raw_fd();
        result.is_ok()
    }

    #[cfg(not(unix))]
    pub fn set_input_buffer(&self, _bytes: usize) -> bool {
        self.socket.is_some()
    }

    /// Set socket send buffer size using socket2 (cross-platform, safe).
    /// Returns false if socket is not bound.
    #[cfg(unix)]
    pub fn set_output_buffer(&self, bytes: usize) -> bool {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return false,
        };
        let fd = socket.as_raw_fd();
        // SAFETY: We're wrapping an existing valid file descriptor
        let sock2 = unsafe { socket2::Socket::from_raw_fd(fd) };
        let result = sock2.set_send_buffer_size(bytes);
        // Prevent closing the fd when sock2 is dropped
        sock2.into_raw_fd();
        result.is_ok()
    }

    #[cfg(not(unix))]
    pub fn set_output_buffer(&self, _bytes: usize) -> bool {
        self.socket.is_some()
    }

    /// Get socket receive buffer size using socket2 (cross-platform, safe).
    /// Returns 0 if socket is not bound.
    #[cfg(unix)]
    pub fn get_input_buffer(&self) -> i32 {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return 0,
        };
        let fd = socket.as_raw_fd();
        // SAFETY: We're wrapping an existing valid file descriptor
        let sock2 = unsafe { socket2::Socket::from_raw_fd(fd) };
        let result = sock2.recv_buffer_size().unwrap_or(0);
        // Prevent closing the fd when sock2 is dropped
        sock2.into_raw_fd();
        result as i32
    }

    #[cfg(not(unix))]
    pub fn get_input_buffer(&self) -> i32 {
        0
    }

    /// Get socket send buffer size using socket2 (cross-platform, safe).
    /// Returns 0 if socket is not bound.
    #[cfg(unix)]
    pub fn get_output_buffer(&self) -> i32 {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return 0,
        };
        let fd = socket.as_raw_fd();
        // SAFETY: We're wrapping an existing valid file descriptor
        let sock2 = unsafe { socket2::Socket::from_raw_fd(fd) };
        let result = sock2.send_buffer_size().unwrap_or(0);
        // Prevent closing the fd when sock2 is dropped
        sock2.into_raw_fd();
        result as i32
    }

    #[cfg(not(unix))]
    pub fn get_output_buffer(&self) -> i32 {
        0
    }

    pub fn allow_broadcasts(&self, status: bool) -> bool {
        let socket = match &self.socket {
            Some(socket) => socket,
            None => return false,
        };
        socket.set_broadcast(status).is_ok()
    }
}
