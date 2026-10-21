# 第三方声明

根目录 [LICENSE](LICENSE) 为 Apache-2.0。第三方代码、模型和素材分别适用其原许可证；未明确授权的文件不能据此推定为 Apache-2.0。

| 内容 | 来源 | 许可与说明 |
| --- | --- | --- |
| Rust 主体 | [pollen-robotics/microduck](https://github.com/pollen-robotics/microduck)，来源版本 `e63db682baeb90cb78c0032dff1c4b9e0b8b64df` | Apache-2.0，见根目录 LICENSE |
| 标定 IMU Python 模块 | Copyright 2026 Marc Duclusaud，见 calibration/imu/lsm6dsv16x_reader.py | GPL-3.0-or-later，见 LICENSES/GPL-3.0-or-later.txt |
| three.js 160、OrbitControls、STLLoader | calibration/static/vendor/ | MIT，原文见 calibration/static/vendor/LICENSE-three.txt |
| 标定显示网格 | xgoduck_rl 的 xgoduck 网格；文件信息见 calibration/static/model/sources.json | 所附来源清单未提供明确再分发许可，不按 Apache-2.0 授权 |
| 动作模型及引用控制流程 | [LuwuDynamics/xgoduck_runtime_arduino](https://github.com/LuwuDynamics/xgoduck_runtime_arduino/tree/8cdbbd84710d856581982c9eaf0d5e2970666232) | 固定版本未附明确许可；模型来源和哈希见 models/manifest.json，不按 Apache-2.0 授权 |
| 网页与标定台自定义代码 | web/、calibration/ | 未附独立完整许可声明，授权归属需确认，不据根 LICENSE 推定第三方授权 |
| 内置曲谱 | `runtime/sounds/scores/wistful.duckscore`、`duck_strut.mid` | 随上游 Rust 音效模块提供；`wistful` 文件注明为原创。Rust 包声明 Apache-2.0，曲谱未附单独许可声明 |
| 终端显示网格与头部几何 | `runtime/robotctl/assets/duck.bin`、`runtime/kinematics/assets/alpha/robot_walk.xml` | 随 Rust 模块提供；文件未附单独素材许可声明，不能仅按 Rust 包许可推定素材授权 |
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
