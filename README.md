<div align="center">

<img src="assets/ropalolyst-analytics.svg" width="168" alt="Ropalolyst Analytics icon">

# Ropalolyst Analytics

**Warframe 蝠力使竞速分析器**

读取 EE.log，自动记录任务成绩与五阶段竞速分段。

<p>
  <a href="https://github.com/nekox41/Ropalolyst-Analytics/releases">下载 Windows 版本</a>
  ·
  <a href="https://github.com/nekox41/Ropalolyst-Analytics/actions">构建状态</a>
</p>

</div>

---

## 简介

Ropalolyst Analytics 是一个轻量级的 Windows 控制台工具。它持续监控 Warframe 的 `EE.log`，根据可确认的日志事件识别蝠力使任务进程，并使用游戏日志时间戳计算成绩。

竞速记录统一为五个阶段：

| 阶段 | 记录内容 |
| --- | --- |
| 跑图 | 任务开始至蝠力使登场 |
| 第一次撞击 | 第一次撞击电塔 |
| 第二次撞击 | 第二次撞击电塔 |
| 第三次撞击 | 第三次撞击电塔 |
| 结算时间 | 第三次撞击至任务成功 |

界面同时显示累计时间、阶段耗时，以及相对历史平均成绩的差值。成绩保存在本地 SQLite 数据库中。

## 使用

1. 从 [Releases](https://github.com/nekox41/Ropalolyst-Analytics/releases) 下载最新 Windows x64 版本。
2. 启动 Warframe，再运行 `ropalolyst-analytics.exe`；也可以先启动分析器。
3. 正常完成蝠力使任务，程序会自动识别日志并记录成绩。
4. 按 `Q` 或 `Esc` 退出。

程序默认自动定位当前用户的 Warframe 日志：

```text
%LOCALAPPDATA%\\Warframe\\EE.log
```

任务成绩数据库会保存在日志文件所在目录：

```text
ropalolyst-analytics.sqlite3
```

## 从源码构建

需要 Rust stable 工具链。在项目根目录执行：

```powershell
cargo build --release -p ropalolyst-cli
```

生成的程序位于 `target/release/ropalolyst-analytics.exe`。

---

<div align="center">
  <sub>Built for consistent runs, clear splits, and better records.</sub>
</div>
