# 第三方声明

根目录 [LICENSE](LICENSE) 为 Apache-2.0。第三方代码、模型和素材分别适用其原许可证；未明确授权的文件不能据此推定为 Apache-2.0。简明来源声明见 [NOTICE](NOTICE)，随应用安装到 `/opt/robot/licenses/NOTICE`；署名不替代再分发授权。

| 内容 | 来源 | 许可与说明 |
| --- | --- | --- |
| Rust 主体 | [pollen-robotics/microduck](https://github.com/pollen-robotics/microduck)，来源版本 `e63db682baeb90cb78c0032dff1c4b9e0b8b64df` | Apache-2.0，见根目录 LICENSE |
| 标定 IMU Python 模块 | Copyright 2026 Marc Duclusaud，见 calibration/imu/lsm6dsv16x_reader.py | GPL-3.0-or-later，见 LICENSES/GPL-3.0-or-later.txt |
| three.js 160、OrbitControls、STLLoader | calibration/static/vendor/ | MIT，原文见 calibration/static/vendor/LICENSE-three.txt |
| 标定显示网格 | [xgoduck_rl](https://github.com/LuwuDynamics/xgoduck_rl/tree/afd091be041caf5e6d4f44102c959e7571b82a37)，15 个 STL 与该版本 xgoduck 资产文件的 Git blob 一致 | 该固定版本根 LICENSE 为 Apache-2.0，具体路径与许可链接见 sources.json；此来源结论不覆盖另行发布的硬件设计与装配文档 |
| 四个上游动作模型及引用控制流程 | [LuwuDynamics/xgoduck_runtime_arduino](https://github.com/LuwuDynamics/xgoduck_runtime_arduino/tree/8cdbbd84710d856581982c9eaf0d5e2970666232) | 固定版本未附明确许可；模型来源和哈希见 models/manifest.json，不按 Apache-2.0 授权 |
| 自训坐下／起立模型 | `models/xgoduck_posture_14999_tspi.onnx`，训练源码 LuwuDynamics/xgoduck_rl 固定版本 `afd091be041caf5e6d4f44102c959e7571b82a37` | 本仓库 Apache-2.0；来源及检查点哈希见 models/manifest.json |
| 网页控制及标定流程 | web/assets/app.js 改编自上述 XgoDuck runtime；标定流程参考其舵机设置与标定实现 | 运行仓库固定版本未附明确许可；不能将这些衍生内容笼统视为独立原创或自动套用 Apache-2.0 |
| 网页主题与交互参考 | [LaoGuaiGe/sbsWeb](https://github.com/LaoGuaiGe/sbsWeb)，首次设置及控制页主题、卡片和表单交互 | 参考项目 LICENSE 为 Apache-2.0；与 XgoDuck 来源内容分别记录 |
| 内置曲谱 | `runtime/sounds/scores/wistful.duckscore`、`duck_strut.mid` | 与 Pollen 固定版本分别为换行归一化后一致、字节一致；上游根 LICENSE 为 Apache-2.0，保留上游文件内声明 |
| 终端显示网格 | `runtime/robotctl/assets/duck.bin` | 与 Pollen 固定版本字节一致；上游根 LICENSE 为 Apache-2.0 |
| 头部几何 | `runtime/kinematics/assets/alpha/robot_walk.xml` | 与记录的 Pollen 固定版本不完全一致，具体修改来源仍需补齐；不因不匹配就认定没有许可 |
| UART3 内核补丁 | Linux 基线 060f864152f60b603a204826c538be3af4d15bff | GPL-2.0-only，见 LICENSES/GPL-2.0-only.txt |
| Cargo 与系统依赖 | runtime/Cargo.lock、deploy/dependencies.lock.json | 各依赖分别适用其原许可证 |

## 随附许可与依赖信息

- [Apache-2.0 正文](LICENSE)
- [GPL-3.0-or-later 正文](LICENSES/GPL-3.0-or-later.txt)：标定台的 IMU Python 模块
- [GPL-2.0-only 正文](LICENSES/GPL-2.0-only.txt)：Linux 内核补丁
- [three.js MIT 正文](calibration/static/vendor/LICENSE-three.txt)
- [Rust 与目标系统依赖清单](deploy/dependencies.lock.json)：列出 ARM64 生产依赖的名称、版本及许可标识；不替代各依赖的许可原文
- [模型来源、接口与完整 SHA256](models/manifest.json)
- [标定网格来源与 SHA256](calibration/static/model/sources.json)

本声明用于说明文件来源与授权范围，不为未明确授权的模型或素材补授许可。再分发这些文件前，应取得相应授权，并保留适用的版权及许可文本。

标定服务在同一 Python 进程中加载 GPL-3.0-or-later 的 IMU 模块，组合程序的发布边界、许可和对应源码义务需按 GPL 单独处理。独立 Rust 服务与系统包不能仅因存放在同一镜像而一概视为相同许可。

当前依赖表记录 169 个 Rust 生产／构建依赖的许可表达式，不等于完整的二进制许可包。分发时仍须补齐适用版权和许可原文、MPL 对应源码取得说明，以及镜像内 Linux、Debian 软件包和 ONNX Runtime 的许可材料。

IMU 接口整理：保留 LSM Python 文件原 GPL-3.0-or-later 署名；BNO085 为依据 CEVA SH-2/SHTP 协议的独立实现。教程固定版本的 `imu_reader.py` 文件自身为 GPL-3.0-or-later（Marc Duclusaud），并非教程根目录的 MIT。参考链接与坐标边界见 [IMU 驱动说明](docs/imu-drivers.md)，未新增第三方依赖。
