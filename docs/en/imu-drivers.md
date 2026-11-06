# IMU drivers and calibration interfaces

**English** | [简体中文](../zh-CN/imu-drivers.md)

## Layers and conventions

`runtime/duck-control/src/imu_driver.rs` defines `Driver`, `Sample`, `Timed`, capabilities, and shared processing. `imu_i2c.rs` / `imu_bno085.rs` handle chips; `scs.rs` takes Reader snapshots without I2C transactions. New chip support does not require changes to policy logic, the 61-value observation, joint behavior, or main daemon initialization.

The calibration equivalent is `calibration/imu/`. `read()` returns only newly received fields, shared `Processor` handles fusion, and `body()` applies installation/bias. Daemon and calibration must exclusively access the device at different times; systemd service conflicts enforce the existing separation.

| Field | Sensor output | Shared output |
| --- | --- | --- |
| quat | Hamilton `[w,x,y,z]`, sensor-to-world | `q_body = q_sensor × inverse(mount)`, torso-to-world |
| gyro | Sensor frame, rad/s | `rotate(mount, gyro - bias_sensor)`, torso rad/s |
| accel | Sensor-frame specific force, m/s², including gravity response | Independent capability/data; missing values are not zero-filled |
| gravity | Derived from orientation | `rotate(inverse(q_body), [0,0,-1])`, unit vector near `[0,0,-1]` upright |

`mount` rotates sensor coordinates into the torso; `gyro_bias` is stored in sensor coordinates. Neither chip fusion nor host fusion hard-codes installation rotation in a driver.

Shared processing derives unit gravity from one quaternion, without independently median-filtering its components. I2C transients may differ from the old reader; physical validation must cover fast tilt and static noise.

Every field carries its own monotonic sample time and presence. Rust uses `Option<Timed<N>>`; Python uses absent keys or `(value, monotonic_seconds)`. Initially no data exists. Gyro reports cannot refresh orientation time, and reading a snapshot refreshes no sample. Daemon IMU readiness requires orientation and gyro both within150 ms, no current read error, and matching installation calibration. Acceleration remains absent if unsupported/unreceived. A live thread/open device does not establish readiness.

LSM SFLP retains at least25 actual FIFO orientation reports for startup stabilization. Without FIFO timestamps, successful host read time is used. Backlog exceeding16 FIFO words per iteration is discarded and processing recovers after draining. BNO corrects reception time with SH-2 base timestamp, rebase, and report delay in100 μs units. Without INT wiring, the host cannot know actual interrupt time; this corrected timestamp cannot measure absolute end-to-end latency. At most8 SHTP cargos are read per iteration; exhausted budget discards that iteration's output and reports backlog, rather than relabeling queued data as fresh. Report/channel sequences are deduplicated separately; malformed/truncated packets never partially commit.

BNO085 I2C reads first fetch a4-byte header, then the full length. The second read may repeat the header or use a continuation header with matching length/channel and sequence incremented by one, including wraparound. Only these matches are accepted. Isolated continuation, changed length/channel, and invalid sequences are rejected. One cargo can contain several16-byte Product ID responses; do not require total cargo length16.

## Hardware configuration

Both programs read `/etc/robot/robotd.toml`; calibration supports `--config /path/robotd.toml`. Invalid configuration fails before hardware access, without probing alternative addresses or replacing models automatically.

An older LSM configuration without `model` defaults to `lsm6dsv16x`, preserving other defaults. TaishanPi example:

```toml
[imu]
source = "i2c"
model = "lsm6dsv16x"
i2c_dev = "/dev/i2c-2"
i2c_addr = 107 # 0x6B; 0x6A is also supported
fusion = "sflp" # chip fusion; comp selects host complementary fusion
```

BNO085 example; LSM `sflp` is not valid for BNO:

```toml
[imu]
source = "i2c"
model = "bno085"
i2c_dev = "/dev/i2c-2"
i2c_addr = 75 # 0x4B; 0x4A is also supported
fusion = "game_rv"
```

Examples do not establish the current robot's installation, wiring, or validation. `lever_arm` (torso-frame meters), `g_filter_k`, and `accel_trust_g` apply only to LSM `comp`. That mode derives initial tilt from actual acceleration, integrates gyro, corrects gravity, and uses SI lever-arm compensation. Its revised algorithm still needs physical comparison.

BNO requests50 Hz Game Rotation Vector, calibrated gyro, and acceleration, checking capabilities through Get Feature replies. Orientation/gyro must be confirmed enabled. Rejected/unconfirmed acceleration is explicitly unsupported without blocking installation/bias calibration. Integration performs no soft reset, tare, DCD save, or nonvolatile chip write. Magnetometer absolute-heading and gyro-integrated channels are not enabled; relative heading can drift.

Linux i2c-dev uses non-forced address selection,100 ms adapter timeout, and zero kernel retries; unsupported constraints fail explicitly. Timeout settings affect the adapter, so both programs must keep exclusive access. Short transactions fail without rereading the remainder as a separate transaction. SHTP fetches a4-byte header then the full cargo with repeated header, at most4096 bytes. Changed headers, excessive length, isolated continuation, or missing/undeclared fragments are rejected.

A BNO header-read NACK can mean no report or disconnection. It is not a new sample; field ages detect stale data, while other transaction errors remain visible. Empty/error reads back off20–500 ms; backlog draining stays bounded at20 ms to avoid worsening queues. I2C never blocks the main controller thread; initialization/reset does not retry forever. Initialization failure stays visible until hardware inspection and service restart. Exit notifies the reader, waits for bounded transactions, and closes descriptors without replacement threads. Kernel/adapter timeout violations require a lower-level fix, not leaked threads.

## Model-specific calibration

Use existing `visual_calibration.json` and IMU UI. Opening the page or selecting a chip does not save mounting or change servo HOME/direction/limits.

Read `imu_calibrations[model]` first, with legacy `imu` compatibility. A legacy entry without model applies only to LSM sensor coordinates; the old calibration used identity delta and no extra axis transform. New records carry `model`, `frame: "sensor-v1"`, and `mount_valid`:

```json
{"imu_calibrations":{"bno085":{"model":"bno085","frame":"sensor-v1","mount_valid":true,"mount":[1,0,0,0],"gyro_bias":[0,0,0]}}}
```

This illustrates structure, not valid physical calibration. Without a matching model record, internal identity rotation may be used with `mount_valid=false`, never valid torso orientation. Static bias collection does not calibrate unknown installation; actively acquire poses, compute rotation, inspect, and save. Editing one model preserves others; `imu` retains the current entry for older tools. Restore matching calibration backups before reverting to older programs that do not understand model-specific entries.

After mounting changes, acquire upright and at least one perpendicular pose, compute/preview, inspect physically, and explicitly save. Sampling windows accept only genuinely new orientation/gyro samples; stale/duplicate snapshots cannot fill sample counts. The page shows model, fusion, connection/errors, capabilities, field ages, and installation validity.

## Add an IMU

1. Implement Rust `imu_driver::Driver`: initialize in the constructor, return new reports in `read`, release resources in `close`. See the [Rust skeleton](../../examples/imu/imu-driver-template.rs). All hardware waits must be bounded.
2. Register construction/configuration in `imu_driver::open` and address/fusion combinations in `robotd-params`. Generic `imu.model` already exists; new chip-specific parameters need new fields.
3. Implement the Python equivalent; register combinations in `imu.MODELS`, construction in `open_driver`, and the model whitelist in calibration mathematics. See the [Python skeleton](../../examples/imu/imu-driver-template.py). Keep mounting rotation out of chip drivers.
4. Verify order, direction, units, and time with identical packets/geometric cases in both implementations. `runtime/duck-control/src/imu_contract_vectors.json` is shared by in-source Rust tests and locally retained Python tests. Test missing/truncated/replayed/stale data, stop, and error exit; do not merely restate formulas in tests.
5. `deploy/install.py` collects `calibration/imu/` source automatically, excluding tests/caches. Update dependencies/licenses for new requirements. Python currently needs3.11 for standard-library `tomllib`; this integration adds no Cargo or third-party Python dependency.

## Physical acceptance remains required

Packet/math tests and ARM64 compilation cannot establish continuous hardware output or physical direction. After separate deployment authorization, test each chip for at least30 minutes, exceeding the historical roughly6-minute stoppage window. Record report rates, ages, errors, and backlog. Unplug/reconnect power and restart services; verify stopped data becomes invalid and servo timing stays unaffected. With the robot supported and policies off, check gyro/tilt/gravity on every axis. Recalibrate mounting/static bias and verify save/restart/chip-switch loads only matching calibration. Active services, Product ID responses, and single samples do not replace these checks.

## Sources and licenses

BNO protocol references are the [CEVA SH-2 manual](https://www.ceva-ip.com/wp-content/uploads/SH-2-Reference-Manual.pdf) and [official CEVA implementation](https://github.com/ceva-dsp/sh2) (Apache-2.0). This repository independently implements Rust/Python drivers without adding third-party source dependencies.

The reviewed tutorial [imu_reader.py](https://github.com/AI-FanGe/Microduck-build-tutorial/blob/49678215d0b40529522772a40c06c216390449dd/microduck/src/imu_reader.py) declares **GPL-3.0-or-later / Copyright2026 Marc Duclusaud**. The tutorial's root MIT license does not override that file. Reference protocol details without copying its mounting, Raspberry Pi bus, servo, policy, or authorization boundaries. Existing LSM Python GPL notices remain; see [third-party notices](../../THIRD_PARTY_NOTICES.md) for the combined calibration service.
