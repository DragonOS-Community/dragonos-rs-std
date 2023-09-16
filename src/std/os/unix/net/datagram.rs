#[cfg(any(doc, target_os = "android", target_os = "linux"))]
use super::{recv_vectored_with_ancillary_from, send_vectored_with_ancillary_to, SocketAncillary};
use super::{sockaddr_un, SocketAddr};
#[cfg(any(doc, target_os = "android", target_os = "linux"))]
use crate::std::io::{IoSlice, IoSliceMut};
use crate::std::net::Shutdown;
use crate::std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::std::path::Path;
use crate::std::sys::cvt;
use crate::std::sys::net::Socket;
use crate::std::sys_common::{AsInner, FromInner, IntoInner};
use crate::std::time::Duration;
use crate::std::{fmt, io};
use dlibc;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "haiku",
    target_os = "nto",
))]
use dlibc::MSG_NOSIGNAL;
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "haiku",
    target_os = "nto",
)))]
const MSG_NOSIGNAL: dlibc::c_int = 0x0;

/// A Unix datagram socket.
///
/// # Examples
///
/// ```no_run
/// use std::os::unix::net::UnixDatagram;
///
/// fn main() -> std::io::Result<()> {
///     let socket = UnixDatagram::bind("/path/to/my/socket")?;
///     socket.send_to(b"hello world", "/path/to/other/socket")?;
///     let mut buf = [0; 100];
///     let (count, address) = socket.recv_from(&mut buf)?;
///     println!("socket {:?} sent {:?}", address, &buf[..count]);
///     Ok(())
/// }
/// ```
pub struct UnixDatagram(Socket);

impl fmt::Debug for UnixDatagram {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixDatagram");
        builder.field("fd", self.0.as_inner());
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        if let Ok(addr) = self.peer_addr() {
            builder.field("peer", &addr);
        }
        builder.finish()
    }
}

impl UnixDatagram {
    /// Creates a Unix datagram socket bound to the given path.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let sock = match UnixDatagram::bind("/path/to/the/socket") {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't bind: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
        pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        unsafe {
            let socket = UnixDatagram::unbound()?;
            let (addr, len) = sockaddr_un(path.as_ref())?;

            cvt(dlibc::bind(socket.as_raw_fd(), &addr as *const _ as *const _, len as _))?;

            Ok(socket)
        }
    }

    /// Creates a Unix datagram socket bound to an address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock1 = UnixDatagram::bind("path/to/socket")?;
    ///     let addr = sock1.local_addr()?;
    ///
    ///     let sock2 = match UnixDatagram::bind_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(err) => {
    ///             println!("Couldn't bind: {err:?}");
    ///             return Err(err);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
        pub fn bind_addr(socket_addr: &SocketAddr) -> io::Result<UnixDatagram> {
        unsafe {
            let socket = UnixDatagram::unbound()?;
            cvt(dlibc::bind(
                socket.as_raw_fd(),
                &socket_addr.addr as *const _ as *const _,
                socket_addr.len as _,
            ))?;
            Ok(socket)
        }
    }

    /// Creates a Unix Datagram socket which is not bound to any address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let sock = match UnixDatagram::unbound() {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't unbound: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
        pub fn unbound() -> io::Result<UnixDatagram> {
        let inner = Socket::new_raw(dlibc::AF_UNIX, dlibc::SOCK_DGRAM)?;
        Ok(UnixDatagram(inner))
    }

    /// Creates an unnamed pair of connected sockets.
    ///
    /// Returns two `UnixDatagrams`s which are connected to each other.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let (sock1, sock2) = match UnixDatagram::pair() {
    ///     Ok((sock1, sock2)) => (sock1, sock2),
    ///     Err(e) => {
    ///         println!("Couldn't unbound: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
        pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (i1, i2) = Socket::new_pair(dlibc::AF_UNIX, dlibc::SOCK_DGRAM)?;
        Ok((UnixDatagram(i1), UnixDatagram(i2)))
    }

    /// Connects the socket to the specified path address.
    ///
    /// The [`send`] method may be used to send data to the specified address.
    /// [`recv`] and [`recv_from`] will only receive data from that address.
    ///
    /// [`send`]: UnixDatagram::send
    /// [`recv`]: UnixDatagram::recv
    /// [`recv_from`]: UnixDatagram::recv_from
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     match sock.connect("/path/to/the/socket") {
    ///         Ok(sock) => sock,
    ///         Err(e) => {
    ///             println!("Couldn't connect: {e:?}");
    ///             return Err(e)
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
        pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unsafe {
            let (addr, len) = sockaddr_un(path.as_ref())?;

            cvt(dlibc::connect(self.as_raw_fd(), &addr as *const _ as *const _, len))?;
        }
        Ok(())
    }

    /// Connects the socket to an address.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let bound = UnixDatagram::bind("/path/to/socket")?;
    ///     let addr = bound.local_addr()?;
    ///
    ///     let sock = UnixDatagram::unbound()?;
    ///     match sock.connect_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(e) => {
    ///             println!("Couldn't connect: {e:?}");
    ///             return Err(e)
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
        pub fn connect_addr(&self, socket_addr: &SocketAddr) -> io::Result<()> {
        unsafe {
            cvt(dlibc::connect(
                self.as_raw_fd(),
                &socket_addr.addr as *const _ as *const _,
                socket_addr.len,
            ))?;
        }
        Ok(())
    }

    /// Creates a new independently owned handle to the underlying socket.
    ///
    /// The returned `UnixDatagram` is a reference to the same socket that this
    /// object references. Both handles can be used to accept incoming
    /// connections and options set on one side will affect the other.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let sock_copy = sock.try_clone().expect("try_clone failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn try_clone(&self) -> io::Result<UnixDatagram> {
        self.0.duplicate().map(UnixDatagram)
    }

    /// Returns the address of this socket.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let addr = sock.local_addr().expect("Couldn't get local address");
    ///     Ok(())
    /// }
    /// ```
        pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { dlibc::getsockname(self.as_raw_fd(), addr, len) })
    }

    /// Returns the address of this socket's peer.
    ///
    /// The [`connect`] method will connect the socket to a peer.
    ///
    /// [`connect`]: UnixDatagram::connect
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.connect("/path/to/the/socket")?;
    ///
    ///     let addr = sock.peer_addr().expect("Couldn't get peer address");
    ///     Ok(())
    /// }
    /// ```
        pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { dlibc::getpeername(self.as_raw_fd(), addr, len) })
    }

    fn recv_from_flags(
        &self,
        buf: &mut [u8],
        flags: dlibc::c_int,
    ) -> io::Result<(usize, SocketAddr)> {
        let mut count = 0;
        let addr = SocketAddr::new(|addr, len| unsafe {
            count = dlibc::recvfrom(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                flags,
                addr,
                len,
            );
            if count > 0 {
                1
            } else if count == 0 {
                0
            } else {
                -1
            }
        })?;

        Ok((count as usize, addr))
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read and the address from
    /// whence the data came.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let mut buf = vec![0; 10];
    ///     let (size, sender) = sock.recv_from(buf.as_mut_slice())?;
    ///     println!("received {size} bytes from {sender:?}");
    ///     Ok(())
    /// }
    /// ```
        pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_flags(buf, 0)
    }

    /// Receives data from the socket.
    ///
    /// On success, returns the number of bytes read.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let mut buf = vec![0; 10];
    ///     sock.recv(buf.as_mut_slice()).expect("recv function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    /// Receives data and ancillary data from socket.
    ///
    /// On success, returns the number of bytes read, if the data was truncated and the address from whence the msg came.
    ///
    /// # Examples
    ///
    #[cfg_attr(any(target_os = "android", target_os = "linux"), doc = "```no_run")]
    #[cfg_attr(not(any(target_os = "android", target_os = "linux")), doc = "```ignore")]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let mut buf1 = [1; 8];
    ///     let mut buf2 = [2; 16];
    ///     let mut buf3 = [3; 8];
    ///     let mut bufs = &mut [
    ///         IoSliceMut::new(&mut buf1),
    ///         IoSliceMut::new(&mut buf2),
    ///         IoSliceMut::new(&mut buf3),
    ///     ][..];
    ///     let mut fds = [0; 8];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     let (size, _truncated, sender) = sock.recv_vectored_with_ancillary_from(bufs, &mut ancillary)?;
    ///     println!("received {size}");
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {fd}");
    ///             }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux"))]
        pub fn recv_vectored_with_ancillary_from(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<(usize, bool, SocketAddr)> {
        let (count, truncated, addr) = recv_vectored_with_ancillary_from(&self.0, bufs, ancillary)?;
        let addr = addr?;

        Ok((count, truncated, addr))
    }

    /// Receives data and ancillary data from socket.
    ///
    /// On success, returns the number of bytes read and if the data was truncated.
    ///
    /// # Examples
    ///
    #[cfg_attr(any(target_os = "android", target_os = "linux"), doc = "```no_run")]
    #[cfg_attr(not(any(target_os = "android", target_os = "linux")), doc = "```ignore")]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let mut buf1 = [1; 8];
    ///     let mut buf2 = [2; 16];
    ///     let mut buf3 = [3; 8];
    ///     let mut bufs = &mut [
    ///         IoSliceMut::new(&mut buf1),
    ///         IoSliceMut::new(&mut buf2),
    ///         IoSliceMut::new(&mut buf3),
    ///     ][..];
    ///     let mut fds = [0; 8];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     let (size, _truncated) = sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     println!("received {size}");
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {fd}");
    ///             }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux"))]
        pub fn recv_vectored_with_ancillary(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<(usize, bool)> {
        let (count, truncated, addr) = recv_vectored_with_ancillary_from(&self.0, bufs, ancillary)?;
        addr?;

        Ok((count, truncated))
    }

    /// Sends data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.send_to(b"omelette au fromage", "/some/sock").expect("send_to function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        unsafe {
            let (addr, len) = sockaddr_un(path.as_ref())?;

            let count = cvt(dlibc::sendto(
                self.as_raw_fd(),
                buf.as_ptr() as *const _,
                buf.len(),
                MSG_NOSIGNAL,
                &addr as *const _ as *const _,
                len,
            ))?;
            Ok(count as usize)
        }
    }

    /// Sends data on the socket to the specified [SocketAddr].
    ///
    /// On success, returns the number of bytes written.
    ///
    /// [SocketAddr]: crate::std::os::unix::net::SocketAddr
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let bound = UnixDatagram::bind("/path/to/socket")?;
    ///     let addr = bound.local_addr()?;
    ///
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.send_to_addr(b"bacon egg and cheese", &addr).expect("send_to_addr function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn send_to_addr(&self, buf: &[u8], socket_addr: &SocketAddr) -> io::Result<usize> {
        unsafe {
            let count = cvt(dlibc::sendto(
                self.as_raw_fd(),
                buf.as_ptr() as *const _,
                buf.len(),
                MSG_NOSIGNAL,
                &socket_addr.addr as *const _ as *const _,
                socket_addr.len,
            ))?;
            Ok(count as usize)
        }
    }

    /// Sends data on the socket to the socket's peer.
    ///
    /// The peer address may be set by the `connect` method, and this method
    /// will return an error if the socket has not already been connected.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.connect("/some/sock").expect("Couldn't connect");
    ///     sock.send(b"omelette au fromage").expect("send_to function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    /// Sends data and ancillary data on the socket to the specified address.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    #[cfg_attr(any(target_os = "android", target_os = "linux"), doc = "```no_run")]
    #[cfg_attr(not(any(target_os = "android", target_os = "linux")), doc = "```ignore")]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary};
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let buf1 = [1; 8];
    ///     let buf2 = [2; 16];
    ///     let buf3 = [3; 8];
    ///     let bufs = &[
    ///         IoSlice::new(&buf1),
    ///         IoSlice::new(&buf2),
    ///         IoSlice::new(&buf3),
    ///     ][..];
    ///     let fds = [0, 1, 2];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     ancillary.add_fds(&fds[..]);
    ///     sock.send_vectored_with_ancillary_to(bufs, &mut ancillary, "/some/sock")
    ///         .expect("send_vectored_with_ancillary_to function failed");
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux"))]
        pub fn send_vectored_with_ancillary_to<P: AsRef<Path>>(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary: &mut SocketAncillary<'_>,
        path: P,
    ) -> io::Result<usize> {
        send_vectored_with_ancillary_to(&self.0, Some(path.as_ref()), bufs, ancillary)
    }

    /// Sends data and ancillary data on the socket.
    ///
    /// On success, returns the number of bytes written.
    ///
    /// # Examples
    ///
    #[cfg_attr(any(target_os = "android", target_os = "linux"), doc = "```no_run")]
    #[cfg_attr(not(any(target_os = "android", target_os = "linux")), doc = "```ignore")]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary};
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let buf1 = [1; 8];
    ///     let buf2 = [2; 16];
    ///     let buf3 = [3; 8];
    ///     let bufs = &[
    ///         IoSlice::new(&buf1),
    ///         IoSlice::new(&buf2),
    ///         IoSlice::new(&buf3),
    ///     ][..];
    ///     let fds = [0, 1, 2];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     ancillary.add_fds(&fds[..]);
    ///     sock.send_vectored_with_ancillary(bufs, &mut ancillary)
    ///         .expect("send_vectored_with_ancillary function failed");
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux"))]
        pub fn send_vectored_with_ancillary(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<usize> {
        send_vectored_with_ancillary_to(&self.0, None, bufs, ancillary)
    }

    /// Sets the read timeout for the socket.
    ///
    /// If the provided value is [`None`], then [`recv`] and [`recv_from`] calls will
    /// block indefinitely. An [`Err`] is returned if the zero [`Duration`]
    /// is passed to this method.
    ///
    /// [`recv`]: UnixDatagram::recv
    /// [`recv_from`]: UnixDatagram::recv_from
    ///
    /// # Examples
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_read_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_read_timeout function failed");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
    ///     let result = socket.set_read_timeout(Some(Duration::new(0, 0)));
    ///     let err = result.unwrap_err();
    ///     assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    ///     Ok(())
    /// }
    /// ```
        pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(timeout, dlibc::SO_RCVTIMEO)
    }

    /// Sets the write timeout for the socket.
    ///
    /// If the provided value is [`None`], then [`send`] and [`send_to`] calls will
    /// block indefinitely. An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method.
    ///
    /// [`send`]: UnixDatagram::send
    /// [`send_to`]: UnixDatagram::send_to
    ///
    /// # Examples
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_write_timeout function failed");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// An [`Err`] is returned if the zero [`Duration`] is passed to this
    /// method:
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
    ///     let result = socket.set_write_timeout(Some(Duration::new(0, 0)));
    ///     let err = result.unwrap_err();
    ///     assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    ///     Ok(())
    /// }
    /// ```
        pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(timeout, dlibc::SO_SNDTIMEO)
    }

    /// Returns the read timeout of this socket.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_read_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_read_timeout function failed");
    ///     assert_eq!(sock.read_timeout()?, Some(Duration::new(1, 0)));
    ///     Ok(())
    /// }
    /// ```
        pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(dlibc::SO_RCVTIMEO)
    }

    /// Returns the write timeout of this socket.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_write_timeout function failed");
    ///     assert_eq!(sock.write_timeout()?, Some(Duration::new(1, 0)));
    ///     Ok(())
    /// }
    /// ```
        pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(dlibc::SO_SNDTIMEO)
    }

    /// Moves the socket into or out of nonblocking mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_nonblocking(true).expect("set_nonblocking function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// Moves the socket to pass unix credentials as control message in [`SocketAncillary`].
    ///
    /// Set the socket option `SO_PASSCRED`.
    ///
    /// # Examples
    ///
    #[cfg_attr(
        any(
            target_os = "android",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "freebsd",
        ),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(any(
            target_os = "android",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "freebsd"
        )),
        doc = "```ignore"
    )]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_passcred(true).expect("set_passcred function failed");
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "freebsd"
    ))]
        pub fn set_passcred(&self, passcred: bool) -> io::Result<()> {
        self.0.set_passcred(passcred)
    }

    /// Get the current value of the socket for passing unix credentials in [`SocketAncillary`].
    /// This value can be change by [`set_passcred`].
    ///
    /// Get the socket option `SO_PASSCRED`.
    ///
    /// [`set_passcred`]: UnixDatagram::set_passcred
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "freebsd"
    ))]
        pub fn passcred(&self) -> io::Result<bool> {
        self.0.passcred()
    }

    /// Set the id of the socket for network filtering purpose
    ///
    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")),
        doc = "```ignore"
    )]
    /// #![feature(unix_set_mark)]
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_mark(32)?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "linux", target_os = "freebsd", target_os = "openbsd",))]
        pub fn set_mark(&self, mark: u32) -> io::Result<()> {
        self.0.set_mark(mark)
    }

    /// Returns the value of the `SO_ERROR` option.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     if let Ok(Some(err)) = sock.take_error() {
    ///         println!("Got error: {err:?}");
    ///     }
    ///     Ok(())
    /// }
    /// ```
        pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// Shut down the read, write, or both halves of this connection.
    ///
    /// This function will cause all pending and future I/O calls on the
    /// specified portions to immediately return with an appropriate value
    /// (see the documentation of [`Shutdown`]).
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    /// use std::net::Shutdown;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.shutdown(Shutdown::Both).expect("shutdown function failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }

    /// Receives data on the socket from the remote address to which it is
    /// connected, without removing that data from the queue. On success,
    /// returns the number of bytes peeked.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recv` system call.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(unix_socket_peek)]
    ///
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::bind("/tmp/sock")?;
    ///     let mut buf = [0; 10];
    ///     let len = socket.peek(&mut buf).expect("peek failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// Receives a single datagram message on the socket, without removing it from the
    /// queue. On success, returns the number of bytes read and the origin.
    ///
    /// The function must be called with valid byte array `buf` of sufficient size to
    /// hold the message bytes. If a message is too long to fit in the supplied buffer,
    /// excess bytes may be discarded.
    ///
    /// Successive calls return the same data. This is accomplished by passing
    /// `MSG_PEEK` as a flag to the underlying `recvfrom` system call.
    ///
    /// Do not use this function to implement busy waiting, instead use `dlibc::poll` to
    /// synchronize IO events on one or more sockets.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// #![feature(unix_socket_peek)]
    ///
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::bind("/tmp/sock")?;
    ///     let mut buf = [0; 10];
    ///     let (len, addr) = socket.peek_from(&mut buf).expect("peek failed");
    ///     Ok(())
    /// }
    /// ```
        pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_flags(buf, dlibc::MSG_PEEK)
    }
}

impl AsRawFd for UnixDatagram {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_inner().as_raw_fd()
    }
}

impl FromRawFd for UnixDatagram {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> UnixDatagram {
        UnixDatagram(Socket::from_inner(FromInner::from_inner(OwnedFd::from_raw_fd(fd))))
    }
}

impl IntoRawFd for UnixDatagram {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.0.into_inner().into_inner().into_raw_fd()
    }
}

impl AsFd for UnixDatagram {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_inner().as_fd()
    }
}

impl From<UnixDatagram> for OwnedFd {
    #[inline]
    fn from(unix_datagram: UnixDatagram) -> OwnedFd {
        unsafe { OwnedFd::from_raw_fd(unix_datagram.into_raw_fd()) }
    }
}

impl From<OwnedFd> for UnixDatagram {
    #[inline]
    fn from(owned: OwnedFd) -> Self {
        unsafe { Self::from_raw_fd(owned.into_raw_fd()) }
    }
}