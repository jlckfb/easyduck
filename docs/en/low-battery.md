# Low-battery sitting

**English** | [简体中文](../zh-CN/low-battery.md)

This behavior runs inside `robotd`; it does not require an open browser or gamepad connection to assess voltage. Existing control-heartbeat, IMU, and bus-loss torque-release rules still apply.

## Configuration

The board profile `config/board/robotd.toml` sets:

```toml
[safety]
low_battery_sit = true
low_battery_voltage = 6.2
battery_empty_shutdown = false
```

The generic default is disabled, with a default threshold of 6.2 V; only the board profile explicitly enables it. A valid, nonzero smoothed voltage at or below the threshold triggers protection. Bus voltage updates approximately once per second, with about 10 seconds of smoothing. This is not an exact measurement of battery capacity.

## Behavior after triggering

- Play one low-battery alert and immediately preempt walking, pickup, roulade, fall recovery, and browser/gamepad control, without waiting for audio.
- If already energized and the model/sensors are available, use the sitting policy (`posture` in XGO, not fall-recovery `sitstand`) and clear other action inputs. Ordinary heartbeats or ownership handoff do not cancel sitting.
- Run the sitting policy for at least 2 seconds, then release torque after measured leg speeds stay below 0.15 rad/s and measured-to-target angle errors below 0.15 rad for 0.5 seconds. At 6 seconds, release torque on timeout and log the reason without claiming successful sitting.
- Release torque if the policy is unavailable or sensors fail; keep retrying failed torque-release writes. An already relaxed robot is not automatically energized.
- Reject ordinary browser/gamepad actions, default-pose requests, enable requests, mode switches, and ordinary sounds until the daemon restarts. Manual torque release remains available; voltage recovery does not clear the latch.
- Do not shut down Linux. While protection is active, reject shutdown requests and bypass the old automatic-shutdown decision.

## Sound bank

Bank version is 6. `sounds ensure-bank` regenerates a bank containing `low_battery/low_battery_a.wav`, retaining the robot's seed. No network or new dependency is required.

Deployment must update `robotd`, `sounds`, board configuration, and the sound bank together. Updating only `robotd` can leave the alert missing. Audio-device/file failures do not block sitting or delay joint control until playback finishes.

## Verification scope

Offline checks cover thresholds, one-time triggering, invalid readings, sitting-state selection, transitions, audio preemption, no low-battery shutdown, and no automatic energizing. Physical sitting and speaker playback still require separately authorized device validation.
