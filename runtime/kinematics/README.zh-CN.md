# kinematics — 姿态几何计算

[English](README.md) | **简体中文**

## 职责

根据内嵌 MJCF 描述与关节角计算几何姿态，并提供头部朝向计算。模型加载时预先整理根节点到目标点的链路，便于反复查询。这是数学库，不读写舵机、不运行 ONNX，也不计算接触动力学。

## 文件结构

| 文件／目录 | 职责 |
| --- | --- |
| [src/lib.rs](src/lib.rs) | Model 解析入口、预编译链、目标点和姿态查询 |
| [src/mjcf.rs](src/mjcf.rs) | XML 刚体／关节／目标点解析 |
| [src/math.rs](src/math.rs) | 位姿／四元数与变换辅助 |
| [src/head.rs](src/head.rs) | 使用同一几何模型的头部朝向计算 |
| [assets/alpha/robot_walk.xml](assets/alpha/robot_walk.xml) | 编译时嵌入的几何描述 |

## 调用与数据流

1. robotd 在支持的朝向／控制计算中使用本库的几何结果。robotctl 使用自己的内嵌网格几何代码，不链接本库。
2. 输入关节角数组与已解析目标点，经预编译链得到空间变换。
3. MJCF 源码被嵌入程序；只改板端 XML 文件不会替换已编译的几何模型。

### 工作区直接关系

- 依赖的本地库：工作区内没有。
- 直接使用方：[robotd](../robotd/README.zh-CN.md)

这里是 Cargo 依赖关系，不代表每条可选运行路径都已启用。

## 边界与修改注意

- 几何描述和关节名称必须匹配目标机器人，XML 中的范围不是实测编码器标定。
- 它不是 MuJoCo 仿真器或训练工程；旧 look 接口也不代表在当前 XGO 头部命令链中具有完全相同的行为。

## 相关说明

- [详细使用说明](../../docs/zh-CN/runtime.md)
- [文档索引](../../docs/zh-CN/README.md)
- [仓库维护规则](../../AGENTS.md)
