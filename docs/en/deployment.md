# Build and deployment

**English** | [简体中文](../zh-CN/deployment.md)

Deployment assets live under `deploy/`:

| File | Purpose |
| --- | --- |
| `Dockerfile` | Fixed Rust toolchain for ARM64 cross-builds from Ubuntu |
| `../.dockerignore` | Pass dependency manifests only; exclude source/configuration/models/private files |
| `build.sh` | Cross-build four ARM64 programs in an existing toolchain container |
| `install.py` | Stage/install application files, preserve configuration, create rollback backups |
| `rollback.py` | Restore files, permissions, and version links from a manifest |
| `start.sh` | Preview by default; `--execute` creates runtime users and starts normal services |
| `dependencies.lock.json` | Reference toolchain, target requirements, Rust production dependency licenses |

## Create a build environment

Use a Linux x86-64 host with Bash, Docker Engine, GNU tar, `sha256sum`, and `file`. See [Docker installation](https://docs.docker.com/engine/install/); the current user needs Docker permission. ARM64 hosts are not the native host for this Dockerfile.

From the repository root:

```sh
docker build -f deploy/Dockerfile -t easyduck-build:1.98.1 .
docker run -d --name easyduck-build --network none easyduck-build:1.98.1
bash deploy/build.sh
```

Image creation downloads Ubuntu, packages, fixed Rust, and dependencies from `runtime/Cargo.lock`. The container has no robot devices, personal-directory mounts, or privileged mode. Application builds use `--locked --offline` with networking disabled.

The Dockerfile pins Ubuntu 22.04's image digest and Rust 1.98.1, and installs ARM64 GCC, target libudev, and native test libraries. Ubuntu package sources are not historical snapshots, so different build dates are not promised byte-identical images. See `deploy/dependencies.lock.json`.

Root `.dockerignore` restricts the build context to Cargo manifests/lockfile. The image contains no robot calibration, policies, application source, or separate local tests. Temporary placeholder modules fetch dependencies; `build.sh` supplies current source for actual compilation. New workspace members must also be listed in `.dockerignore`.

Rust is under `/root/.cargo/bin/`; the cross-compiler is `aarch64-linux-gnu-gcc`. Ubuntu multiarch installs ARM64 libraries in `/usr/lib/aarch64-linux-gnu/`; `/opt/gst-sysroot` links to `/` for existing sysroot parameters.

### Reuse and dependency updates

Start a stopped container with `docker start easyduck-build`; do not repeat a same-name `docker run`. Rebuild the image after toolchain/dependency changes and create a new container:

```sh
docker build -f deploy/Dockerfile -t easyduck-build:updated .
docker run -d --name easyduck-build-updated --network none easyduck-build:updated
EASYDUCK_BUILD_CONTAINER=easyduck-build-updated bash deploy/build.sh
```

Offline builds cannot fetch missing dependencies. Creating the image needs access to Ubuntu, Rust, and crates.io, without relying on another machine's Cargo cache.

If official Rust downloads are slow, follow [RsProxy guidance](https://rsproxy.cn/) without changing global settings:

```sh
docker build -f deploy/Dockerfile -t easyduck-build:1.98.1 \
  --build-arg RUSTUP_DIST_SERVER=https://rsproxy.cn \
  --build-arg RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup .
```

These arguments change Rust download addresses, not the fixed toolchain or Cargo lockfile.

Custom containers need equivalent toolchain, target libraries, and cached dependencies. A recorded container image ID identifies a validation environment, not a pullable image address; use the Dockerfile to build from scratch.

### Hardware-free unit tests

From the repository root, use the same image for native Linux tests, with read-only source and temporary outputs:

```sh
docker run --rm --network none \
  --mount type=bind,src="$(pwd)/runtime",dst=/work/runtime,readonly \
  -e CARGO_TARGET_DIR=/tmp/easyduck-tests -w /work/runtime \
  easyduck-build:1.98.1 cargo test --locked --offline --workspace
```

Do not pass serial, I2C, or other robot devices. Unit tests do not establish physical-motion acceptance.

## Build outputs

`build/bin/` contains `robotd`, `padd`, `robotctl`, and `sounds`, accompanied by build logs, file types, and SHA256 manifests. Builds use separate source directories while sharing dependency caches.

| Variable | Default or purpose |
| --- | --- |
| `EASYDUCK_BUILD_CONTAINER` | `easyduck-build` |
| `EASYDUCK_OUTPUT_DIR` | Repository `build/`; override for another destination |
| `EASYDUCK_CARGO_CACHE` | Container `/tmp/easyduck-target` |
| `EASYDUCK_SYSROOT` | Container `/opt/gst-sysroot` |
| `EASYDUCK_CARGO` | Container `/root/.cargo/bin/cargo` |
| `EASYDUCK_CROSS_CC` | Container `aarch64-linux-gnu-gcc` |

## Stage an installation

```sh
python3 deploy/install.py --root build/stage --profile examples
```

This stages programs, manifest-listed models, web/calibration assets, settings, service units, audio configuration, licenses, and notices. Both profiles are uncalibrated templates; `board` supplies TaishanPi hardware references. Build distributable images from an empty stage or explicitly refreshed image templates; never copy a used robot's `/etc/robot/` as defaults.

`/opt/robot/licenses/THIRD_PARTY_NOTICES.md` is installed, hashed, and rolled back with the application. Source-relative links must be viewed in this repository. three.js licensing remains with browser assets.

Existing `/etc/robot/` files are preserved unless `--replace-config` explicitly replaces/backups them. A complete rollback manifest is written before replacement; copy errors report its location.

The managed `opt/robot/daemon/current` symlink points to `.`. An earlier symlink target is backed up. If it is a real directory/file, arrange existing versions before installation.

## Install on a board

Copy installation source and four ARM64 binaries to the board. Binaries default to repository `build/bin/`, or use `--binaries DIR`. The target needs dependencies listed by the Python lockfiles, ONNX Runtime 1.23+, ALSA, libudev, BlueZ, and UART3 half-duplex kernel support. The installer handles application files only.

Support the robot and stop control/related services before running on the board:

```sh
sudo python3 deploy/install.py --root / --profile examples --install-on-board
```

The output identifies a backup directory with `var/lib/easyduck/backups/<ID>/manifest.json`. Verify installed files without starting control:

```sh
sudo python3 deploy/install.py --root / --verify /var/lib/easyduck/backups/<ID>/manifest.json
```

## Startup

```sh
bash deploy/start.sh
sudo bash deploy/start.sh --execute
```

The first previews; the second creates required users, reloads systemd, and starts daemon/gamepad/controller services. Behavior depends on configuration; finish calibration and support the robot first. Calibration is a separate, mutually exclusive service.

For boot startup, explicitly run `systemctl enable robotd.service padd.service xgoduck-web.service`.

## Rollback

Stop related services, then use the installer's backup directory:

```sh
sudo python3 deploy/rollback.py --root / --backup /var/lib/easyduck/backups/<ID> --restore-on-board
```

Rollback restores files, permissions, and links, removing files added by that installation. Start services separately when appropriate. For a temporary stage, change `--root` / `--backup` and omit `--restore-on-board`.

`deploy/systemd/`, `deploy/sysusers.d/`, and `deploy/alsa/` are installation inputs. `deploy/kernel/` includes the UART3 patch for its matching baseline and gamepad configuration/check inputs. The target system supplies kernel, ONNX Runtime, and system dependencies. Build/file verification is not physical acceptance.

## First connection setup

Setup uses NetworkManager and BlueZ for Wi-Fi/PS5, reachable through USB, wireless, or Ethernet addresses. See [connection setup](connection-setup.md) for separate installation, reconnection, and acceptance steps.

## USB / 2.4 GHz gamepads

See [receiver connection, kernel configuration, and image verification](usb-gamepad.md). Application installation does not replace a matching xpad module build; SDK packaging verifies modules, boot loading, and USB aliases.
