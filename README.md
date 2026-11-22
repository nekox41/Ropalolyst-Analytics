# 蝠力使竞速分析器（Ropalolyst Analytics）

一个使用 Rust 编写的 Warframe 蝠力使（Ropalolyst）任务日志分析工具。第一版采用 Windows 控制台界面，自动读取游戏日志，识别任务与 Boss 战相关事件，并记录任务成绩。

## 第一版目标

- 自动定位当前 Windows 用户的 `%LOCALAPPDATA%\\Warframe\\EE.log`。
- 增量读取日志，跟踪游戏运行期间追加的内容，并处理日志文件截断或重建。
- 识别任务开始、成功、失败和结束边界，按任务会话分割记录。
- 使用 EE.log 的时间戳计算成绩，不使用分析器启动时间或读取时间。
- 在中文终端界面显示当前任务、计时、事件时间线、日志状态和历史成绩。
- 识别蝠力使登场动画、三座电塔的撞击动画、复位动画及击杀后的落地动画。
- 识别有脚本/资源映射依据的传讯事件，并保留其真实语义与可信度。
- 将完成的任务记录保存在本地 SQLite 数据库，计算最佳成绩和平均成绩。
- 通过 GitHub Actions 构建独立 Windows x64 可执行文件并发布 Release。

## 当前事件识别边界

### 任务与动画

- `SS_STARTED` 状态转换：任务开始。
- `SS_STARTED -> SS_ENDING`：任务结束边界；如果没有先识别到成功或失败，则记为中止候选。
- `RopalolystGasArenaIntro01_cin.fbx`：蝠力使登场动画。
- `TowerCrashA_cin.fbx`：撞击电塔动画。日志实例 `CrashCinematic0/1/2` 对应地图中的电塔 1/2/3。**电塔编号不是撞塔轮次**；实际轮次应按本次任务里事件发生的先后顺序计算。
- `TowerCrashEnd_cin.fbx`：电塔复位动画。
- `TennoLand_cin.fbx`：击杀后的落地动画。它使用 `ResetCinematic1`，不能按普通电塔复位动画过滤。

### 已映射的传讯事件

以下传讯资源来自 `RopalolystTransmissionSet` 的静态配置映射。程序识别的是“脚本请求播放的传讯”，并不自动将其等同于动作完成。

| 传讯资源后缀 | 程序显示语义 | 不能据此推断 |
|---|---|---|
| `DShieldDownAladV` | 护盾提示 | 不是破盾完成；可能重复 |
| `DOperRemoveShield0190Natah` | 首次破盾信号 | 仅对应资料中描述的特定条件分支 |
| `DCapaChargeAladV` | 电塔充能提醒 | 不代表充能完成 |
| `DShootWeakPntsAladV` | 弱点提示 | 不能直接作为永久击破计数 |
| `DRopaLandPlatAladV` | 最后弱点提示 | 对应脚本中剩余弱点为 0 的分支 |
| `DMntRopa0400AladV` | 最后一轮骑乘尝试 | 请求发生在实际骑乘操作之前，不证明成功 |
| `DRopaStunAladV` | 眩晕提示 | 不能用出现次数推断永久击破的弱点数 |
| `DHitStunRopaNatah` | 平台激光命中结果 | 是激光处理结果，不是开关按下时刻 |
| `DLaserMissAladV` | 平台激光未命中结果 | 是激光处理结果，不是开关按下时刻 |
| `DRopaKillLaserNatah` | 平台激光处决结果（候选） | 时间戳不等于 Boss 血量归零或死亡帧 |

### 暂不声称可精确识别的内容

目前静态逆向资料尚未确认以下动作具有独立、可靠的正常 EE.log 签名：

- 电梯按钮交互的准确时刻；
- 通用骑乘交互成功；
- 每一轮破盾完成的准确时刻；
- 平台激光开关按下的准确时刻；
- 弱点永久破坏的逐个计数；
- 电塔/诱饵实际充能完成时刻。

因此，第一版不会通过动画开始时间倒推按钮时刻，也不会用传讯提醒冒充完成事件。静态资源和 Lua 脚本分析证明的是调用语义；仍需使用实际游戏日志验证运行时日志形态及不同游戏版本的一致性。

> 逆向证据适用范围：目前资料来自指定 Steam 安装资源与已有 Lua 反编译稿，不能自动保证不同版本 OpenWF 客户端具有完全相同的日志行为。

## 竞速分段与历史对比

控制台以 `MissionStarted` 的日志时间戳为 0.000 秒，展示每个阶段的：

- **累计时间**：当前标记相对任务开始的时间。
- **阶段耗时**：当前标记与上一个有效分段标记之间的时间差。
- **较历史平均**：当前阶段耗时减去历史成功任务中相同分段的平均耗时。负数表示快于平均，正数表示慢于平均；没有可比较历史数据时显示横线。

第一版分段点：

1. 跑图结束：蝠力使登场动画开始。
2. Boss 出场结束：登场动画停止。
3. 第 1/2/3 次撞塔：按本次任务中 `CrashCinematic` 事件出现顺序编号，同时显示实际电塔 ID。
4. 撞塔动画结束：对应 `CrashCinematic` 的 `Stop`。
5. 返回中央动画开始与结束：分别记录 `TowerCrashEnd_cin.fbx` 的 `Start` 和 `Stop`。
6. 激光处决结果：收到 `DRopaKillLaserNatah` 传讯。
7. 处决后落地开始与结束：分别记录 `TennoLand_cin.fbx` 的 `Start` 和 `Stop`。
8. 任务成功：收到任务成功日志。

阶段对比只匹配相同分段标记。历史平均仅使用成功任务；旧成绩缺少某个标记时，不会把它当作 0 秒参与平均。事件未出现或时间顺序异常时，不会生成负耗时分段。

## 技术实现

- **Rust**：日志读取、事件解析、任务状态和可执行程序。
- **Ratatui + Crossterm**：中文终端界面。
- **SQLite**：本地任务成绩历史。
- **Serde**：结构化事件数据序列化。
- **GitHub Actions**：Windows x64 自动构建、测试与 Release 发布。

程序不依赖 Python、Node.js、浏览器内核或桌面 GUI 运行时。

## 项目结构

- `crates/ropalolyst-core`：日志定位与增量读取、解析器、事件模型、任务分段、统计和 SQLite 持久化。
- `crates/ropalolyst-cli`：控制台界面及程序入口。

控制台界面不直接解析 EE.log 原始文本，也不自行猜测游戏状态；它只消费核心模块输出的结构化事件。

## 构建

安装 Rust stable 工具链后，在仓库根目录执行：

```powershell
cargo build --release -p ropalolyst-cli
```

生成文件：

```text
target/release/ropalolyst-analytics.exe
```

## 使用

1. 启动 Warframe，或在游戏启动前运行分析器。
2. 程序自动定位当前用户的 `EE.log`，读取已有日志并持续监控新增内容。
3. 任务结束后，成绩会保存到日志目录旁的 `ropalolyst-analytics.sqlite3`。
4. 按 `Q` 或 `Esc` 退出。

## 数据与隐私

Warframe 日志可能包含账号标识、IP 地址、设备信息和本地路径。程序只在本地保存结构化任务事件和计时数据；写入 SQLite 前会清除事件的原始日志行。请勿提交未经脱敏的玩家日志或录像。用于测试的日志样本应替换个人标识，同时保留事件顺序与时间戳。

## 开发检查

```powershell
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release -p ropalolyst-cli
```
