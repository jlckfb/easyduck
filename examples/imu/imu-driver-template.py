"""Template only. Register in imu.MODELS and imu.open_driver after implementation."""
class ExampleImu:
    def __init__(self, config):
        self.capabilities = dict(orientation=False, gyro=False, accel=False)
        # Open bounded transport, verify identity, enable only supported reports.
        raise NotImplementedError("implement device initialization")

    def read(self):
        # e.g. {"gyro": ([gx, gy, gz], monotonic_sample_time)}; omit missing fields.
        # Quaternion wxyz sensor->world, gyro rad/s, acceleration m/s².
        raise NotImplementedError("implement report decoding")

    def close(self):
        # Release the descriptor even after partially failed initialization.
        pass
