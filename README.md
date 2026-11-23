<div align="center">

<img src="logo.png" width="240" alt="Ropalolyst Analytics — aa project artwork">

# Ropalolyst Analytics

**Warframe 蝠力使竞速分析器**

轻量、实时的 EE.log 竞速计时工具。

[下载 Windows 版本](https://github.com/nekox41/Ropalolyst-Analytics/releases)　·　[查看构建状态](https://github.com/nekox41/Ropalolyst-Analytics/actions)

</div>

---

## 关于项目

Ropalolyst Analytics 自动监控 Warframe 日志，识别蝠力使任务中的关键事件，并以任务开始时刻为计时基准，记录整场成绩。

竞速过程被整理为五个清晰的阶段：

| 阶段 | 计时内容 |
|:--|:--|
| 跑图 | 任务开始 → 蝠力使登场 |
| 第一次撞击 | 跑图结束 → 第一次撞击电塔 |
| 第二次撞击 | 第一次 → 第二次撞击电塔 |
| 第三次撞击 | 第二次 → 第三次撞击电塔 |
| 结算时间 | 第三次撞击 → 任务成功 |

程序会同时展示累计时间、阶段耗时，以及相较历史平均成绩的差值。成绩保存在本地，无需联网。\n\n检测到任务开始后，程序还会在屏幕顶部显示一个置顶、透明且鼠标穿透的实时计时 Overlay；计时起点通过日志启动时的 UTC 时间锚点校准，Overlay 只显示总计时，不显示阶段信息。

## 使用方法

1. 前往 [Releases](https://github.com/nekox41/Ropalolyst-Analytics/releases) 下载 Windows x64 版本并解压。
2. 启动 Warframe，运行 `ropalolyst-analytics.exe`（也支持先启动分析器）。
3. 正常完成蝠力使任务，程序会自动识别并保存成绩。
4. 按 `Q` 或 `Esc` 退出。

默认日志位置：

```text
%LOCALAPPDATA%\Warframe\EE.log
```

成绩数据库 `ropalolyst-analytics.sqlite3` 保存在日志文件所在目录。

## 从源码构建

需要 Rust stable 工具链和 Python 3。先生成 Windows 图标，再构建：

```powershell
python -m pip install Pillow
python scripts/generate_icon.py
cargo build --release -p ropalolyst-cli
```

生成的可执行文件位于 `target/release/ropalolyst-analytics.exe`。

---

<div align="center">
<sub>Made with care · Built for consistent runs</sub>
</div>
