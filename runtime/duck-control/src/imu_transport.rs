//! Bounded Linux I²C transactions. Never force ownership or retry short transfers.
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
pub struct I2c {
    fd: Option<OwnedFd>,
}
impl I2c {
    pub fn open(dev: &str, addr: u8) -> io::Result<Self> {
        let path = std::ffi::CString::new(dev).map_err(io::Error::other)?;
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let bus = Self {
            fd: Some(unsafe { OwnedFd::from_raw_fd(fd) }),
        };
        // Adapter timeout is in 10 ms units; no kernel retries. Refuse unsupported bounds.
        for (op, arg) in [(0x0702, 10), (0x0701, 0), (0x0703, i32::from(addr))] {
            if unsafe { libc::ioctl(fd, op as libc::c_ulong, arg as libc::c_ulong) } < 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(bus)
    }
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        let fd = self
            .fd
            .as_ref()
            .ok_or_else(|| io::Error::other("closed I2C"))?
            .as_raw_fd();
        let n = unsafe { libc::write(fd, data.as_ptr().cast(), data.len()) };
        Self::exact(n, data.len())
    }
    pub fn read(&self, n: usize) -> io::Result<Vec<u8>> {
        let fd = self
            .fd
            .as_ref()
            .ok_or_else(|| io::Error::other("closed I2C"))?
            .as_raw_fd();
        let mut v = vec![0; n];
        let got = unsafe { libc::read(fd, v.as_mut_ptr().cast(), n) };
        Self::exact(got, n)?;
        Ok(v)
    }
    fn exact(n: isize, want: usize) -> io::Result<()> {
        if n < 0 {
            Err(io::Error::last_os_error())
        } else if n as usize != want {
            Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short I2C transaction",
            ))
        } else {
            Ok(())
        }
    }
    pub fn close(&mut self) -> io::Result<()> {
        self.fd.take();
        Ok(())
    }
}
