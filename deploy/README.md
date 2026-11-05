# 编译与部署

| 文件 | 用途 |
| --- | --- |
| `Dockerfile` | 从 Ubuntu 基础镜像创建固定 Rust 版本的 ARM64 编译环境 |
| `../.dockerignore` | 仅传入依赖清单，排除源码、配置、模型及私有文件 |
| `build.sh` | 在已有 Docker 工具链容器中交叉编译四个 ARM64 程序 |
| `install.py` | 生成安装目录或安装到系统根目录，保留已有配置并创建回退备份 |
| `rollback.py` | 根据备份清单恢复文件、权限和版本链接 |
| `start.sh` | 默认预览启动命令；`--execute` 创建运行用户并启动正式服务 |
| `dependencies.lock.json` | 参考工具链、目标依赖要求及 Rust 生产依赖的许可信息 |

## 从零搭建编译环境

编译机使用 Linux x86-64，需要 Bash、Docker Engine、GNU tar、`sha256sum` 和 `file`。Docker 安装方式见[官方说明](https://docs.docker.com/engine/install/)；当前用户需有运行 Docker 的权限。ARM64 主机不是本 Dockerfile 的原生构建环境。

在仓库根目录执行：

```sh
docker build -f deploy/Dockerfile -t easyduck-build:1.98.1 .
docker run -d --name easyduck-build --network none easyduck-build:1.98.1
bash deploy/build.sh
```

第一步需要联网下载 Ubuntu 镜像、系统包、固定版本 Rust 和 `runtime/Cargo.lock` 对应的依赖。第二步创建只用于编译的容器，不映射开发板设备、不挂载个人目录，也不需要特权模式。容器关闭网络后，应用仍通过 `--locked --offline` 编译。

Dockerfile 固定 Ubuntu 22.04 基础镜像摘要和 Rust 1.98.1，安装 ARM64 GCC、目标平台 libudev 及本机测试所需的开发库。系统包来自 Ubuntu 官方更新源，未固定为历史快照，因此不承诺不同日期构建的镜像逐字节相同。版本记录见 `dependencies.lock.json`。

构建上下文由根目录 `.dockerignore` 限定，只传入 Cargo 工作区清单和锁文件；镜像不包含机器人标定、模型、业务源码或独立本地测试。Cargo 下载依赖时使用临时占位模块，正式编译时由 `build.sh` 传入当前源码。增加工作区模块时需同步更新 `.dockerignore` 中的模块目录清单。

容器内 Rust 位于 `/root/.cargo/bin/`，编译器为 `aarch64-linux-gnu-gcc`。Ubuntu 多架构包把 ARM64 库安装在 `/usr/lib/aarch64-linux-gnu/`；`/opt/gst-sysroot` 是指向根目录的兼容链接，供现有脚本的 sysroot 参数使用。

### 容器复用与依赖更新

容器已存在且停止时使用 `docker start easyduck-build`；不要重复执行同名 `docker run`。更换工具链或修改 Cargo 依赖后，重建镜像并使用新名称创建容器：

```sh
docker build -f deploy/Dockerfile -t easyduck-build:updated .
docker run -d --name easyduck-build-updated --network none easyduck-build:updated
EASYDUCK_BUILD_CONTAINER=easyduck-build-updated bash deploy/build.sh
```

本地镜像或容器中没有的依赖，不能在离线编译时自动获取。创建镜像需能访问 Ubuntu、Rust 和 crates.io，不依赖旧机器上的 Cargo 缓存。

Rust 官方下载源较慢时，可按 [RsProxy 说明](https://rsproxy.cn/)选择镜像源，不改系统或个人的全局配置：

```sh
docker build -f deploy/Dockerfile -t easyduck-build:1.98.1 \
  --build-arg RUSTUP_DIST_SERVER=https://rsproxy.cn \
  --build-arg RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup .
```

这两个参数只改变 Rust 安装下载地址，不更改固定工具链版本或 Cargo 锁文件。

自备容器也须提供对应工具链、目标库和依赖缓存。清单中的容器镜像 ID 是验证环境标识，不是可拉取的镜像地址；从零创建环境应使用 Dockerfile。

### 无实物单元测试

在仓库根目录使用同一镜像执行 Linux 本机测试，源码只读挂载，产物写入临时容器：

```sh
docker run --rm --network none \
  --mount type=bind,src="$(pwd)/runtime",dst=/work/runtime,readonly \
  -e CARGO_TARGET_DIR=/tmp/easyduck-tests -w /work/runtime \
  easyduck-build:1.98.1 cargo test --locked --offline --workspace
```

不向容器传入串口、I2C 或其他设备。单元测试不代表实物动作验收。

## 编译输出

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

这会把程序、清单中列出的模型、网页、标定台、配置、服务文件、声卡配置、许可证及第三方声明放到 `build/stage/`。两套配置均为新机器待标定模板；`board` 提供泰山派硬件接口参考。打包通用镜像时应使用空安装目录或明确更新镜像构建目录中的模板，不能把已使用机器的 `/etc/robot/` 当成默认配置复制进去。

`/opt/robot/licenses/THIRD_PARTY_NOTICES.md` 随应用安装，纳入哈希校验与回退；其中指向源码的相对链接需在软件仓库中查看。three.js 许可仍随标定台浏览器素材提供。

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

## 首次配网与手柄连接

连接设置页通过 NetworkManager 和 BlueZ 配置 Wi-Fi 与 PS5 手柄，可使用设备的 USB、无线或以太网地址访问。独立安装、自动重连及验收步骤见 [连接设置说明](setup/README.md)。

## USB / 2.4G gamepads

[接收器连接、内核配置与镜像验收](../docs/gamepad-usb.md)。应用安装不代替匹配内核的 xpad 模块构建；SDK 打包时会检查模块、启动加载规则及 USB 别名。
