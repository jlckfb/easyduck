//! Template only: register the constructor in imu_driver::open after implementing it.
use std::io;
use duck_control::imu_driver::{Capabilities, Driver, I2cImuConfig, Sample};
pub struct ExampleImu;
impl ExampleImu {
    pub fn open(_config: &I2cImuConfig) -> io::Result<Self> {
        // Open the transport with bounded timeouts, verify identity, enable reports.
        Err(io::Error::new(io::ErrorKind::Unsupported, "implement device initialization"))
    }
}
impl Driver for ExampleImu {
    fn capabilities(&self) -> Capabilities { Capabilities::default() }
    fn read(&mut self) -> io::Result<Sample> {
        // Return only real newly received Timed values in sensor coordinates.
        Err(io::Error::new(io::ErrorKind::Unsupported, "implement report decoding"))
    }
    fn close(&mut self) -> io::Result<()> { Ok(()) }
}
