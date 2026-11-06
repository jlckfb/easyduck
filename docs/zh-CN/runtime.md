# Rust 运行程序

[English](../en/runtime.md) | **简体中文**

完整 Cargo 工作区，编译入口为 `deploy/build.sh`，依赖版本由 `Cargo.lock` 锁定。

| 类型 | 模块 | 作用 |
| --- | --- | --- |
| 程序 | robotd | 50Hz 主控制、动作切换、状态发布 |
| 程序 | padd | 将手柄输入送给主控 |
| 程序 | robotctl | 状态查询、命令行操作和诊断 |
| 程序 | sounds | 生成机器人音效库 |
| 共用模块 | duck-control | HD1910 通信、I2C IMU、模型推理和控制保护 |
| 共用模块 | duck-ipc-proto | 客户端与主控的消息格式及关节顺序 |
| 共用模块 | robotd-params | 配置解析与参数定义 |
| 共用模块 | kinematics | 姿态几何计算 |

四个共用模块编入程序，不需要分别启动服务。`runtime/test-fixtures/` 是测试输入，不是板端部署配置。

`runtime/duck-control/tests/fixtures/scs_calibration_test_fixture.json` 是舵机自动测试专用的固定样本，不参与部署；安装基线模板位于 `config/board/zero_pos.json`，它是待标定占位数据；每台实物使用自身 `/etc/robot/visual_calibration.json`。

构建、测试和安装步骤见[软件使用说明](software-guide.md)。
