<div align="center">

<img src="logo.png" width="210" alt="Ropalolyst Analytics · aa">

# Ropalolyst Analytics

**Warframe 蝠力使竞速分析器 · Rust × Tauri 2**

轻量、实时、专注每一次竞速表现。

[下载 Windows 版本](https://github.com/nekox41/Ropalolyst-Analytics/releases/latest) · [构建状态](https://github.com/nekox41/Ropalolyst-Analytics/actions)

</div>

---

## 简介

Ropalolyst Analytics 自动读取 Warframe 的 `EE.log`，以确定的日志事件记录蝠力使刺杀成绩。全新的桌面版采用与 **PT Analytics Next** 相近的设计：现代化中文界面、清晰的时间卡片，以及无需联网的本地历史记录。

| 功能 | 说明 |
|:--|:--|
| **主页** | 最近一场成绩、累计计时、五阶段用时 |
| **历史记录** | 搜索、排序、重命名、收藏、删除、复制成绩 |
| **统计分析** | 最佳成绩、平均用时、阶段平均、成绩排行 |
| **游戏内 Overlay** | 屏幕顶部的实时透明计时器，可调整位置、尺寸和透明度 |
| **数据安全** | SQLite 本地存储，保留旧版记录，删除后不会被同一份日志重新导入 |

竞速始终采用五个稳定阶段：

**跑图 → 第一次撞击 → 第二次撞击 → 第三次撞击 → 结算时间**

不注入游戏、不读取游戏内存。成绩分段采用日志时间戳；Overlay 使用日志启动时的 UTC 时间锚点与系统单调时钟对齐。

## 使用方法

1. 在 [Releases](https://github.com/nekox41/Ropalolyst-Analytics/releases/latest) 下载 Windows x64 安装版或便携版。
2. 启动 Warframe 并打开本程序，软件会自动定位 `EE.log`。
3. 进入蝠力使任务即可开始计时，任务结束后自动保存成绩。
4. 在 **历史记录** 中管理成绩；在 **设置** 中调整游戏内 Overlay。

默认日志：`%LOCALAPPDATA%\Warframe\EE.log`

沿用旧版 SQLite 数据库：`%LOCALAPPDATA%\Warframe\ropalolyst-analytics.sqlite3`

便携版需要 Microsoft Edge WebView2 Runtime；安装版可由安装程序处理运行时依赖。

## 从源码构建

需要 Windows、Rust stable、Node.js 22+、Python 3 和 WebView2。

```powershell
python -m pip install Pillow
python scripts/generate_icon.py
npm install
npm run check
npm run build -- --bundles nsis
```

如果仍然偏好旧版控制台程序，可继续使用：

```powershell
cargo build --release -p ropalolyst-cli
```

---

<div align="center"><sub>Made with care · Built for consistent runs</sub></div>
