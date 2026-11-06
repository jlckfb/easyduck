# Servo and IMU workbench

**English** | [简体中文](../zh-CN/calibration.md)

A Python 3.11+ standard-library service, default port 8766. IMU model, bus, address, and fusion come from the same `/etc/robot/robotd.toml` as the daemon; override with `--config`. LSM6DSV16X and BNO085 are supported; see [IMU integration](imu-drivers.md). Local three.js assets show the robot and support HOME, direction, limits, and IMU calibration.

Defaults are serial `/dev/ttyS3`, baseline `/etc/robot/zero_pos.json`, and saved profile `/etc/robot/visual_calibration.json`. Calibration conflicts with `robotd`, `padd`, and `xgoduck-web`: one program must own serial at a time. Operations may energize servos, move the robot, or write settings; support the robot first. UI labels currently remain Chinese.

## Workbench operations

The compact workbench shows one preparation/range/direction panel beside 3D. Whole-body checks are separate; instructions/manual values are collapsed by default. Switching panels sends no hardware commands.

### Individual joints

1. **Preparation**: select a joint, release torque, and manually match the 3D reference. **清圈并清除旧范围** clears temporary turns and resets this joint's old range without changing other joints, HOME, direction, hardware center, or IMU. **设中位并记录HOME** permanently sets this servo's hardware center and records the position as software HOME using the existing backend; success opens range setup. Do not repeat HOME capture at the same position.
2. **Range**: move manually to both endpoints and record lower/upper counts. **保存范围，核对方向** syncs drafts, confirms/saves this joint, and opens direction setup; other joints/IMU drafts are not saved. 0–4095 is only the encoder range.
3. **Direction**: compare the 3D positive-direction example with one physical positive step. Keep direction when matched; otherwise reverse and recheck. **保存并下一关节** advances only after save succeeds; the final joint opens whole-body checks. Failure does not advance or resend. Observe mouth opening/closing directly because it has no moving 3D joint.

Manual HOME / separate cleanup retains HOME-only, temporary-turn-only, and current-range-only actions. Step size, speed, and direction values are in details. Fine-tuning an existing robot's HOME does not require resetting permanent hardware center.

### Whole-body checks

After all joints, record the whole-body reference, then return to it. Recording only reads/saves positions; returning energizes and moves. Separate buttons distinguish them. Inspect total torso tilt and forward/lateral deviations, and verify the physical robot is vertical.

For correction, expand joint fine-tuning, choose a joint, make small steps, record HOME, save, and return to check. Finally record the body reference again. Unknown IMU installation or stale feedback prevents a verticality conclusion. Live 3D uses current HOME/direction/fresh feedback; without a recorded reference, absolute pose is for comparison only.

### IMU and other operations

The IMU page acquires/saves separately. More options include undo, save all drafts, export, and all-joint feedback. Stop motion and release all torque remain at the top.

All page regions share one status-reading flow. Network errors and individual missing-servo feedback are distinct; communication timeouts do not automatically resend writes. Errors can be copied and dismissed.

## Hardware-free preview

Run in the repository's `calibration/` directory:

```sh
mkdir -p ../build
python3 server.py --mock --bind 127.0.0.1 --http-port 8766 --baseline ../config/examples/zero_pos.json --profile ../build/mock-profile.json --dry-run-apply
```

Open `http://127.0.0.1:8766`; the command uses a mock bus. `--dry-run-apply` only prints service-switch commands.

`calibration/imu/` contains GPL-3.0-or-later IMU readers. See [third-party notices](../../THIRD_PARTY_NOTICES.md) for meshes/browser dependencies.

`/etc/robot/calibration_speed_restore.json` records original servo speeds for restoration on exit.

## Page navigation

Enter from setup or control. [Connection setup](connection-setup.md) coordinates navigation; pages retain the wireless host and read setup-port settings from `/etc/easyduck/setup.env`.

Before leaving, save/discard drafts, stop physical motion, and release all torque; unresolved conditions keep the page open with guidance. Exit freezes new calibration operations, closes serial, and restores temporary speeds before normal services resume. Navigation does not automatically save calibration, energize, or run policies.

## New robot: assign servo IDs

Choose the new-robot ID entry in setup or the ID-assignment tab. The entry stops normal/gamepad control, does not require all 15 expected IDs, and does not automatically energize. Returning to setup keeps calibration's serial ownership and normal control stopped.

1. Existing uniquely numbered servos can remain connected. Add only one factory-ID1 servo at a time and scan IDs0–253. Scans can stop while retaining discoveries. This page uses 1 Mbps; other baud rates are out of scope.
2. Select a discovered ID and target joint: right leg10–14, left leg20–24, neck/head/mouth30–34. Inspect raw position and torque state.
3. Explicitly hold the selected servo at its current position and gently verify the physical joint; do not force it. Continuous readback reports unknown state on disconnect and updates after reconnect. Rescan/selection remain read-only while held. Disconnect never automatically retries torque-on or ID writes. Release the selected servo, confirm only one uses the old ID, tick confirmation, then write/verify the target. The new ID must be free; 254/255 are reserved.
4. Success verifies the new address, no response at the old address, torque off, and restored storage lock. Add the next ID1 servo and repeat. Scanning cannot reliably distinguish duplicate IDs.
5. Reconnect the complete robot, stop scanning, and verify HOME, directions, limits, and IMU in posture calibration. Do not run policies using another robot's calibration.

Duplicate IDs require one-at-a-time wiring. Checksum failures or servo alarms stop scans rather than count as absent devices. ID changes do not write P/D, current limits, target positions, or center; page entry/scans write no parameters. Only the explicit hold action energizes. Release restores temporary speed before permitting an ID write. Before/after records default to `/var/lib/easyduck/servo-id-changes.jsonl`, overridden with `--id-journal`. Interrupted writes/readback do not report success; rescan actual IDs. Reverse an ID change with the same isolated procedure.

Interaction follows [official XGO Duck first-time servo setup](https://github.com/LuwuDynamics/xgoduck_runtime_arduino#first-time-servo-setup), implemented with this project's TaishanPi serial backend.

Simulate one factory-ID1 servo on a PC, from the repository root:

```sh
python3 calibration/server.py --mock --mock-servo-id 1 --bind 127.0.0.1 --http-port 18766 --baseline config/examples/zero_pos.json --profile /tmp/easyduck-id-preview.json --dry-run-apply
```

This accesses no physical serial port. `--mock-servo-id` requires `--mock`.

## First adjustment of a new robot

Images/existing settings may belong to another robot. Process preparation, range, and direction one joint at a time. Turn clearing/current-range reset share an operation; hardware center/HOME capture share a UI step. Range reset backs up and saves only this joint's reset, retaining other joints/drafts without movement.

After manual endpoint capture, confirm/save the current range; no separate save button is needed. Check model direction, then save the joint.

In HD1910 single-turn position mode, manual movement may leave negative/multi-turn feedback. With torque off and single-turn hardware ranges checked, torque-on preparation clears temporary turns, reads position/torque again, then continues the original energizing flow. It does not modify HOME, direction, or EEPROM. Uncleared out-of-range targets produce explicit errors rather than silently writing unexecutable positions.

## Zero crossing and initial hardware center

Actual position is signed and continuous; single-turn position is modulo4096. Near zero they differ: actual−1 maps to4095. Clearing temporary turns does not change encoder zero; do not treat crossing zero as failed turn clearing.

For initial installation near encoder zero, release torque, manually position the selected joint at its proper installation center, then choose the no-motion hardware-center action. This **permanently changes this servo's hardware center**: MID0x0B five times at least10 ms apart, then wait100 ms before readback. It differs from temporary-turn RESET0x0A. See [official calibration](https://github.com/LuwuDynamics/xgoduck_runtime_arduino#calibration); this page keeps torque off and does not automatically perform the official hold step.

Before writing, record raw position, raw center-register bytes, ranges, gains, and software settings in calibration-backups. Success requires center near2048, torque still off, and unchanged range/gain/mode. Other servos are untouched. Recapture/check this joint's HOME/range, confirm direction/save, then redo the body reference. A software backup cannot automatically undo permanent hardware center; reposition/recalibrate when needed.

### Saving ranges

- **Save current joint** saves only its HOME, direction, active range, and confirmation state. Other joint/IMU drafts remain unsaved.
- **Save all** saves all joint and IMU drafts; use it on the IMU page.
- Both save configuration with backups, without changing torque or returning joints. Stop motion before saving. Returning to normal control still requires torque off and saved/discarded remaining drafts.
