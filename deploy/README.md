# 编译与部署

| 文件 | 用途 |
| --- | --- |
| `build.sh` | 在已有 Docker 工具链容器中交叉编译四个 ARM64 程序 |
| `install.py` | 生成安装目录或安装到系统根目录，保留已有配置并创建回退备份 |
| `rollback.py` | 根据备份清单恢复文件、权限和版本链接 |
| `start.sh` | 默认预览启动命令；`--execute` 创建运行用户并启动正式服务 |
| `dependencies.lock.json` | 参考工具链、目标依赖要求及 Rust 生产依赖的许可信息 |

## 编译环境

在 Linux 编译机运行，需要 Bash、Docker、GNU tar、`sha256sum` 和 `file`。先准备一个运行中的编译容器，默认名为 `easyduck-build`，也可通过 `EASYDUCK_BUILD_CONTAINER` 选择其他容器。容器需具备：

- Rust 和 Cargo，参考版本为 `dependencies.lock.json` 中的工具链版本。
- `aarch64-unknown-linux-gnu` Rust 目标和 `aarch64-linux-gnu-gcc`。
- ARM64 sysroot，默认放在 `/opt/gst-sysroot`，其中包含目标平台 libudev 的库和 pkg-config 描述。
- `runtime/Cargo.lock` 对应的 Cargo 依赖缓存；构建使用 `--locked --offline`。

锁文件中的容器镜像 ID 是参考环境标识，不是可拉取的镜像地址。

```sh
EASYDUCK_BUILD_CONTAINER=easyduck-build bash deploy/build.sh
```

输出为 `build/bin/` 下的 `robotd`、`padd`、`robotctl`、`sounds`，同时生成构建日志、文件类型和 SHA256 清单。每次构建使用独立源码目录，依赖缓存复用。

| 环境变量 | 默认值或作用 |
| --- | --- |
| `EASYDUCK_BUILD_CONTAINER` | `easyduck-build` |
| `EASYDUCK_OUTPUT_DIR` | 仓库内 `build/`，可指定其他输出目录 |
| `EASYDUCK_CARGO_CACHE` | 容器内 `/tmp/easyduck-target` |
| `EASYDUCK_SYSROOT` | 容器内 `/opt/gst-sysroot` |
| `EASYDUCK_CARGO` | 容器内 `/root/.cargo/bin/cargo` |
| `EASYDUCK_CROSS_CC` | 容器内 `aarch64-linux-gnu-gcc` |

## 准备安装目录

```sh
python3 deploy/install.py --root build/stage --profile examples
```

这会把程序、清单中列出的模型、网页、标定台、配置、服务文件、声卡配置和许可文件放到 `build/stage/`。`examples` 适用于新机器；`board` 是特定机器的参考配置，使用前应核对自身标定。

已有的 `/etc/robot/` 配置默认保留。显式指定 `--replace-config` 才覆盖它们，旧文件仍进入备份。文件替换前会记录完整回退清单；复制失败时，错误信息会给出该清单的位置。

脚本管理的 `opt/robot/daemon/current` 链接指向 `.`，已有版本链接的原目标会保存在备份中。若该位置是实际目录或普通文件，需要先安排好原版本目录再安装。

## 安装到开发板

将仓库安装源及四个 ARM64 程序放到板端，程序默认位于仓库内 `build/bin/`，也可用 `--binaries DIR` 指定。目标系统需具备 Python 依赖文件中列出的包、ONNX Runtime 1.23 或以上、ALSA、libudev、BlueZ 和 UART3 半双工内核支持；安装器只负责应用文件。

机器人放稳、停止控制和相关服务后，在板端执行：

```sh
sudo python3 deploy/install.py --root / --profile examples --install-on-board
```

命令输出备份目录，其清单位于 `var/lib/easyduck/backups/<编号>/manifest.json`。安装后可只读核对文件：

```sh
sudo python3 deploy/install.py --root / --verify /var/lib/easyduck/backups/<编号>/manifest.json
```

## 启动

```sh
bash deploy/start.sh
sudo bash deploy/start.sh --execute
```

第一条预览命令，第二条创建服务所需用户、重新加载 systemd 并启动主控、手柄和正式网页。实际机器人行为取决于当前配置，启动前需完成标定和摆放准备。标定台是单独的服务，与正式控制服务互斥。

需要开机启动时，可自行执行 `systemctl enable robotd.service padd.service xgoduck-web.service`。

## 回退

停止相关服务后，使用安装时输出的备份目录：

```sh
sudo python3 deploy/rollback.py --root / --backup /var/lib/easyduck/backups/<编号> --restore-on-board
```

回退恢复原文件、权限及版本链接，并删除该次安装新增的文件。完成后按需要启动服务。临时安装目录使用同样命令，替换 `--root` 和 `--backup`，省略 `--restore-on-board`。

`systemd/`、`sysusers.d/` 和 `alsa/` 是安装素材；`kernel/` 是按对应内核基线应用的 UART3 补丁。内核、ONNX Runtime 和系统依赖需由目标系统提供。编译及文件核对通过不能代替实物动作验收。
