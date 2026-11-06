# IMU 驱动与标定接口

[English](../en/imu-drivers.md) | **简体中文**

## 分层与约定

`runtime/duck-control/src/imu_driver.rs` 定义 `Driver`、`Sample`、`Timed`、能力与共用处理；`imu_i2c.rs` 和 `imu_bno085.rs` 只访问芯片。`scs.rs` 只从后台 Reader 取快照，不做 I²C 事务。新增型号无需修改策略、61 维输入、关节业务或主控初始化逻辑。

标定台对应 `calibration/imu/`：驱动的 `read()` 返回仅在本次收到的字段，共用 `Processor` 处理融合，`body()` 处理安装与偏置。主控与标定台必须分时独占同一设备；systemd 标定服务保留原有服务互斥关系。

| 字段 | 传感器输出 | 共用层输出 |
| --- | --- | --- |
| quat | `[w,x,y,z]`，Hamilton 四元数，传感器到世界 | `q_body = q_sensor × inverse(mount)`，躯干到世界 |
| gyro | 传感器坐标，rad/s | `rotate(mount, gyro - bias_sensor)`，躯干坐标 rad/s |
| accel | 传感器坐标，比力 m/s²，包含重力响应 | 原始能力与数据独立报告；不把缺失数据补零 |
| gravity | 从姿态计算 | `rotate(inverse(q_body), [0,0,-1])`，单位向量，直立约 `[0,0,-1]` |

`mount` 是传感器到躯干的安装旋转；`gyro_bias` 保存在传感器坐标。芯片内融合和主机融合都不把安装方向写死在驱动中。

共用层从同一份四元数计算单位重力，不再对重力三个分量各自取中值；I²C 路径的瞬态数值与旧读取层可能有差异，实物验收需覆盖快速倾斜与静止噪声。

每个字段各有单调时钟时间和存在性。Rust 用 `Option<Timed<N>>`，Python 用缺省键及 `(value, monotonic_seconds)`；初始没有数据。只收到角速度不能刷新姿态时间，读取快照不能刷新任何时间。姿态与角速度均在 150 ms 内、当前无读取错误且安装标定匹配时，主控才报告 IMU 可用；保留原有控制安全流程。线程存在或设备文件已打开不代表就绪。加速度不支持或未收到时保持缺失。

LSM SFLP 保留启动时至少 25 个实际 FIFO 姿态报告的稳定等待；无 FIFO 时间戳的姿态使用成功读取时的主机时间；超过每轮 16 个 FIFO 字的积压会被丢弃，排空后恢复。BNO 使用 SH-2 的 base timestamp、rebase 与各报告 delay（100 μs）修正接收时间；没有 INT 引脚时主机无法获知真正的中断时刻，时间是接收时间的协议修正，不能用于测量绝对端到端延迟。每轮最多读取 8 个 SHTP cargo；预算耗尽时丢弃该轮输出并报告积压，不把排队旧数据标成新鲜。报告序号与通道序号分别去重，异常与截断报文不会部分提交。

BNO085 的 I²C 分段读取：先读 4 字节报头，再读完整长度；第二次可返回相同报头，或长度和通道相同、序号递增一（含回绕）的续包报头。只接受这两种匹配关系，孤立续包、长度变化、通道变化及错误序号均拒绝。Product ID 可在一个 cargo 中包含多条 16 字节响应，不应强制整个 cargo 恰好为 16 字节。

## 硬件配置

两端均读取 `/etc/robot/robotd.toml`；标定台可用 `--config /path/robotd.toml` 指定另一份。配置错误在访问硬件前拒绝，不探测其他地址或自动替换型号。

现有 LSM 配置不含 `model` 时，默认 `lsm6dsv16x`，其余原默认值保留。泰山派示例：

```toml
[imu]
source = "i2c"
model = "lsm6dsv16x"
i2c_dev = "/dev/i2c-2"
i2c_addr = 107 # 0x6B；也支持 0x6A
fusion = "sflp" # 芯片融合；或 comp 主机互补融合
```

BNO085 示例（不要把 LSM 的 sflp 用于 BNO）：

```toml
[imu]
source = "i2c"
model = "bno085"
i2c_dev = "/dev/i2c-2"
i2c_addr = 75 # 0x4B；也支持 0x4A
fusion = "game_rv"
```

这些是配置示例，不代表当前机器人已安装、已接线或已验证。`lever_arm`（躯干坐标米）、`g_filter_k`、`accel_trust_g` 仅用于 LSM `comp`；在其他融合模式不参与计算。`comp` 从实际加速度确定初始倾斜，以角速度积分并校正重力方向，杆臂补偿统一使用 SI 单位。此模式算法整理后仍需实物对照验证。

BNO 请求 50 Hz Game Rotation Vector、已校准角速度和加速度，以 Get Feature 回复确认能力。必须确认姿态与角速度成功开启，加速度若被固件拒绝或未确认则明确报告不支持；不阻断安装方向和偏置标定。不进行软复位、tare、DCD 保存或芯片非易失写入。当前接入不启用磁力计绝对航向模式或 gyro-integrated channel；相对航向可漂移。

Linux i2c-dev 使用非强制地址选择、100 ms 适配器超时和零内核重试；适配器不支持这些约束时明确失败。超时设置属于总线适配器，要求两端按现有服务互斥关系使用。短事务直接报错，不把剩余数据当作另一个 I²C 事务重读。SHTP 先读 4 字节头，再按完整长度读取含重复头的完整 cargo（最多 4096 字节），头改变、超长、孤立 continuation 都拒绝；完整 cargo 请求下不拼接未声明或缺失的片段。

BNO 在读取头时出现 NACK 可以表示没有待发报告，也可能是拔线，因此不把它当新样本，并以各字段年龄判定停更；其他事务错误仍明确上报。后台读到空数据或通信错误会以 20～500 ms 退避；有数据积压时保持 20 ms 的有界排空周期，避免退避反而让积压永远追不上。不在主控线程等待 I²C，不无限初始化/复位重试。初始化失败保持可见错误；检查硬件后重启相关服务重新初始化。退出通知读取线程，等待有界事务返回并关闭文件描述符，不创建替代线程。Linux 内核或适配器若不遵守超时仍需修复底层驱动，不能用遗留线程掩盖。

## 型号关联的标定

沿用 `visual_calibration.json` 和原 IMU 页面。只打开页面或切换型号不会保存安装方向，也不会改舵机零点、方向、限位。

优先读取 `imu_calibrations[model]`；兼容旧 `imu` 段。没有型号的旧 `imu` 仅作为 LSM 的传感器坐标标定（旧标定服务调用使用 identity delta，没有额外轴变换）。新记录带 `model`、`frame: "sensor-v1"` 与 `mount_valid`。字段示例：

```json
{"imu_calibrations":{"bno085":{"model":"bno085","frame":"sensor-v1","mount_valid":true,"mount":[1,0,0,0],"gyro_bias":[0,0,0]}}}
```

以上仅示意结构，不能直接作为实物安装标定。未找到匹配型号时，内部计算可使用单位安装矩阵，但 `mount_valid=false`，不会将其当作有效躯干姿态。先静止采集偏置不会把未知安装方向变成已标定；必须主动采集多姿态、计算安装方向并核对后保存。编辑任一型号时会保留原型号条目，`imu` 保留当前条目供旧工具读取。回退旧程序前必须恢复相应版本的标定备份，旧程序不理解型号关联。

改变安装方向：使用原页面采集站直和至少一个垂直姿态，计算、预览和人工核对，然后由用户保存。采样窗口仅接受姿态和角速度都推进的实际新报告，过期或重复快照不能凑够样本数。页面显示型号、融合方式、连接/错误、能力、各字段年龄与安装标定状态。

## 添加一种 IMU

1. Rust 实现 `imu_driver::Driver`，构造函数完成初始化，`read` 仅返回新报告，`close` 释放资源。参考 [Rust 最小模板](../../examples/imu/imu-driver-template.rs)。所有硬件等待必须有界。
2. 在 `imu_driver::open` 注册构造函数和配置组合，在 `robotd-params` 的 IMU 校验登记地址及融合模式；参数登记表已有通用 `imu.model`，新增专属参数才需扩充配置字段。
3. Python 实现相同约定，在 `imu.MODELS` 登记配置组合、`open_driver` 注册构造函数，标定数学层的型号白名单一起登记。参考 [Python 最小模板](../../examples/imu/imu-driver-template.py)。不要将装配旋转放入驱动。
4. 用两端同一组报文和几何样例验证顺序、方向、单位和时间。固定共享样例在 `runtime/duck-control/src/imu_contract_vectors.json`，Rust 源码内单元测试与本地保留的 Python 测试共同读取；不要让测试只是重复实现公式。必须额外覆盖缺失、截断、序号重放、过期、停止与错误退出。
5. `deploy/install.py` 自动收集 `calibration/imu/` 的源码，排除测试和缓存；新增运行依赖需更新依赖清单与许可证。当前 Python 最低 3.11，使用标准库 `tomllib`，没有新增 Cargo 或第三方 Python 依赖。

## 实物验收仍需执行

本地报文/数学测试和 ARM64 编译不能证明硬件连续输出或物理方向正确。部署另行授权后，两种型号分别验证：至少 30 分钟连续采样（超过历史约六分钟停更窗口），记录各报告速率、年龄、错误与积压；拔线、恢复供电、重启服务时检查停更立即变为无效且舵机循环不受阻；在承托、无策略的条件下逐轴核对角速度、倾斜和重力方向；重新采集安装方向和静止偏置，检查保存/重启/切换型号后只加载匹配标定。不要用服务 active、产品查询成功或单次样本代替这些验收。

## 来源与许可

BNO 协议依据 [CEVA SH-2 手册](https://www.ceva-ip.com/wp-content/uploads/SH-2-Reference-Manual.pdf) 与 [CEVA SH-2 官方实现](https://github.com/ceva-dsp/sh2)（其源码 Apache-2.0）；本仓库使用独立的 Rust/Python 实现，没有新增第三方代码依赖。

核对的教程文件是 [Microduck-build-tutorial/imu_reader.py](https://github.com/AI-FanGe/Microduck-build-tutorial/blob/49678215d0b40529522772a40c06c216390449dd/microduck/src/imu_reader.py)。该文件自身声明 **GPL-3.0-or-later / Copyright 2026 Marc Duclusaud**，不能因教程根目录为 MIT 就把文件当 MIT。仅参考协议，不复制其装配旋转、树莓派总线、舵机、策略或授权边界。现有 LSM Python 文件保留原 GPL 声明；组合标定服务的发布边界沿用 [第三方声明](../../THIRD_PARTY_NOTICES.md)。
