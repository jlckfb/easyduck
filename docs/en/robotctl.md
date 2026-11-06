# robotctl command-line tool

**English** | [简体中文](../zh-CN/robotctl.md)

Call the robot service through a Unix socket to query status, view live data, and issue commands. A local parameter editor is also included. `robotd` owns the servo bus; `robotctl` does not directly open serial or IMU devices.

## Build

Run inside the repository's `runtime/` directory:

```sh
cargo build --locked --release -p robotctl
```

It depends on workspace crates `duck-ipc-proto` (protocol), `robotd-params` (parameter definitions/validation). Terminal mesh geometry is implemented locally in `runtime/robotctl/src/duck.rs`, without linking the kinematics crate. Copying this directory alone is insufficient. Runtime requires Linux/Unix sockets; the terminal UI requires an interactive terminal.

## Queries and monitoring

These commands query state without requesting torque or motion:

```sh
robotctl --help
robotctl --version
robotctl robot mode --json
robotctl monitor
robotctl monitor --hz 10 --json
```

The default daemon socket is `/run/robotd.sock`; override it with `--robot-socket`. `monitor --json` writes one JSON object per line and can be redirected. Ctrl-C exits.

The monitor shows daemon state, policy, joint targets/feedback, IMU, power information, and a terminal 3D pose. Raw gamepad events require `padd`'s event stream.

| Monitor key | Action |
| --- | --- |
| `q` / `Esc` / `Ctrl-C` | Exit |
| Up/down, `j` / `k` | Scroll joints |
| `PageUp` / `PageDown` / `Home` | Page or return to the beginning |
| `u` | Change angle units |
| `p` / `d` | Toggle gamepad / 3D display |
| Left/right, `[` / `]` | Rotate the 3D view |

`runtime/robotctl/assets/duck.bin` is a terminal display mesh embedded at compile time, not a policy. No separate runtime copy is needed. The view reflects feedback and does not replace physical direction checks.

## Parameter editing

```sh
sudo robotctl configure --file /etc/robot/robotd.toml
```

Up/down selects, Space toggles, Enter edits, `u` restores the selected default, and `q` starts exit. Saving validates against daemon parameter definitions, preserves untouched comments/order/unknown fields, and atomically replaces the configuration file. Directory write permission is required.

After saving, the UI offers relevant service restarts. Support the robot before restarting control. Editing parameters for an optional service does not install it.

## Commands and service boundaries

Use `robotctl <command> --help` for arguments.

| Command | Behavior and requirements |
| --- | --- |
| `robot init` | Request torque and return to the initial pose; causes motion |
| `robot relax --yes` | Request torque release; an unsupported body loses support |
| `robot do <skill>` | Request an action, subject to daemon state and policy availability |
| `robot look <x> <y> <z>` | Request head direction; torso forward/left/up coordinates in meters |
| `quack` | Request a quack |
| `chorale` | Request configured chorale; may move head/mouth |
| `health` / `version` | Aggregate several services; a missing service alone does not indicate a servo fault |
| `net` / `system` / `pad` | Retained clients requiring external `configd`, not included here |
| `update` | Retained client requiring external `updaterd`, not included here |
| `completions <shell>` | Print completion scripts for current commands |

Actual gamepad control uses `padd`. The retained `pad pair` client cannot replace its external pairing service. A marked legacy Bluetooth compatibility path may also pause `btd` and reset the adapter.

## Exit codes

`0`: success; `1`: execution failure; `2`: invalid arguments; `3`: service unreachable; `4`: update busy; `5`: request rejected; `6`: insufficient permission. Argument-error help follows the current program output.

## Tests

Run in `runtime/`:

```sh
cargo test --locked -p robotctl
```

In-source tests cover parsing, mock communication, parameter saving, and display logic. They require no robot and do not establish correct physical motion.
