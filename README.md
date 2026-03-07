# Deribit BTC Options Arbitrage Scanner

[English](#english) | [中文](#中文)

---

<a id="english"></a>

## English

A real-time BTC options arbitrage detection system for Deribit exchange. Connects via WebSocket, monitors all BTC option instruments, and scans for mispricing opportunities using multiple arbitrage strategies.

### Features

- **Real-time WebSocket streaming** — auto-reconnect, heartbeat, rate limiting
- **7 arbitrage/signal detectors** — from risk-free structural arbs to statistical IV anomalies
- **Terminal UI (TUI)** — interactive dashboard with filtering, sorting, and detail views
- **Offline monitor mode** — review historical opportunities from the database without live connection
- **SQLite persistence** — all instruments, tickers, and opportunities stored locally
- **Portfolio optimizer** — finds hedged combinations that reduce margin and boost APY
- **Leverage-adjusted APY** — annualized return calculation with configurable leverage

### Architecture

```
┌─────────────┐     ┌──────────────┐     ┌───────────────────┐
│  Deribit WS  │────▶│  Event Bus   │────▶│  Analysis Engine  │
│  (streaming) │     │  (broadcast) │     │  (7 strategies)   │
└─────────────┘     └──────┬───────┘     └────────┬──────────┘
                           │                      │
                    ┌──────▼───────┐     ┌────────▼─────────┐
                    │ Ticker Cache │     │   Opportunities   │
                    │ OrderBook    │     │   → SQLite DB     │
                    └──────────────┘     │   → TUI Display   │
                                        │   → Console Alert  │
                                        └──────────────────┘
```

**Modules:**

| Module | Description |
|--------|------------|
| `ws/` | WebSocket client, authentication, rate limiter |
| `market/` | Instrument registry, ticker cache, orderbook, subscriber |
| `analysis/` | 7 strategy analyzers + opportunity model + portfolio optimizer |
| `events/` | Broadcast event bus |
| `storage/` | SQLite persistence layer |
| `alert/` | Console notifier |
| `tui.rs` | Ratatui-based terminal dashboard |
| `bin/monitor.rs` | Offline DB viewer |

### Arbitrage Strategies

#### Structural Arbitrage (Risk-Free)

| Strategy | Description | Risk |
|----------|------------|------|
| **Put-Call Parity** | Detects C - P ≠ 1 - K/S mispricing (BTC-settled) | Low |
| **Box Spread** | 4-leg pure option arb, guaranteed USD payoff = K2 - K1 | Low |
| **Conversion/Reversal** | Synthetic forward vs spot, locks in riskless profit | Low |
| **Vertical Arb** | Monotonicity/convexity violations in strike-price ordering | Low |
| **Calendar Arb** | Far-month < near-month price for same strike (hard constraint) | Low |

#### Statistical Signals (Directional)

| Strategy | Description | Risk |
|----------|------------|------|
| **Vol Surface Anomaly** | Butterfly/pairwise IV outliers via z-score detection | Medium-High |
| **Calendar Spread** | Abnormal IV term structure across expirations | Medium-High |

### Quick Start

#### Prerequisites

- Rust 1.70+ (install via [rustup](https://rustup.rs/))
- Deribit API credentials (get from [test.deribit.com](https://test.deribit.com/) or [deribit.com](https://www.deribit.com/))

#### 1. Clone & Configure

```bash
git clone <repo-url>
cd deribit
cp .env.example .env
# Edit .env with your API credentials
```

#### 2. Build

```bash
cargo build --release
```

#### 3. Run Live Scanner

```bash
cargo run --release
```

This starts the main engine: connects to Deribit, loads all BTC option instruments, subscribes to ticker streams, and scans for arbitrage every 10 seconds. Opportunities are printed to console and saved to SQLite.

#### 4. Run TUI Monitor (optional)

In a separate terminal:

```bash
cargo run --release --bin monitor
```

This opens an interactive dashboard that reads from the database. Works even while the main scanner is running.

### TUI Keybindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `Enter` | View opportunity detail |
| `Esc` / `Backspace` | Back to list |
| `1`-`9` | Switch filter (All / Arbitrage / Signal / PCP / Spread / Conv / Calendar / Vol / Portfolio) |
| `s` | Cycle sort mode (Profit / Time / APY) |
| `l` | Cycle leverage (1x / 2x / 5x / 10x) |

### Configuration

All configuration is via environment variables (`.env` file):

| Variable | Description | Default | Required |
|----------|------------|---------|----------|
| `DERIBIT_CLIENT_ID` | API client ID | — | Yes |
| `DERIBIT_CLIENT_SECRET` | API client secret | — | Yes |
| `DERIBIT_ENV` | `test` for testnet, `prod` for mainnet | `test` | No |
| `ALERT_THRESHOLD` | PCP arbitrage threshold (fraction of underlying, e.g. `0.005` = 0.5%) | `0.005` | No |
| `DB_PATH` | SQLite database file path | `deribit.db` | No |
| `RUST_LOG` | Log level: `trace`, `debug`, `info`, `warn`, `error` | `info` | No |

#### Strategy Parameters (hardcoded, modify in `src/main.rs`)

| Parameter | Strategy | Default | Description |
|-----------|----------|---------|-------------|
| `alert_threshold` | Put-Call Parity | `0.005` | Min profit as fraction of underlying |
| `min_profit_usd` | Box Spread | `10.0` | Min USD profit to trigger |
| `min_profit_usd` | Conversion/Reversal | `10.0` | Min USD profit to trigger |
| `min_profit_usd` | Vertical Arb | `5.0` | Min USD profit to trigger |
| `min_profit_usd` | Calendar Arb | `5.0` | Min USD profit to trigger |
| `butterfly_z_threshold` | Vol Surface | `~2.0` (from `15.0` constructor) | Z-score for butterfly anomaly |
| `min_iv_diff` | Calendar Spread | `10.0` | Min IV percentage point difference |

#### Internal Constants

| Constant | Value | Description |
|----------|-------|-------------|
| Heartbeat interval | 30s | WebSocket keepalive |
| Scan interval | 10s | Time between arbitrage scans |
| Ticker save rate | 1/100 | Save every 100th ticker to DB |
| Event bus capacity | 4096 | Broadcast channel buffer |
| Initial delay | 30s | Wait before first arb scan |
| Instrument refresh | 3600s | Reload instruments hourly |
| Leverage options | 1x, 2x, 5x, 10x | TUI leverage presets |

### Project Structure

```
deribit/
├── Cargo.toml
├── .env.example
├── src/
│   ├── main.rs                 # Live scanner entry point
│   ├── lib.rs                  # Module exports
│   ├── config.rs               # Environment config loader
│   ├── tui.rs                  # Terminal UI dashboard
│   ├── bin/
│   │   └── monitor.rs          # Offline DB viewer entry point
│   ├── ws/
│   │   ├── client.rs           # WebSocket manager & RPC client
│   │   ├── auth.rs             # HMAC-SHA256 authentication
│   │   └── rate_limiter.rs     # API rate limiting
│   ├── market/
│   │   ├── instruments.rs      # Instrument registry & parsing
│   │   ├── ticker.rs           # Real-time ticker cache
│   │   ├── orderbook.rs        # Order book manager
│   │   ├── subscriber.rs       # Channel subscription helper
│   │   └── trades.rs           # Trade data types
│   ├── analysis/
│   │   ├── opportunity.rs      # Opportunity & TradeLeg models
│   │   ├── portfolio.rs        # Portfolio combination optimizer
│   │   ├── put_call_parity.rs  # Put-Call Parity arb
│   │   ├── box_spread.rs       # Box Spread arb
│   │   ├── conversion.rs       # Conversion/Reversal arb
│   │   ├── vertical_arb.rs     # Vertical spread arb
│   │   ├── calendar_arb.rs     # Calendar arb (hard constraint)
│   │   ├── vol_surface.rs      # IV surface anomaly detector
│   │   └── calendar_spread.rs  # Calendar spread (IV signal)
│   ├── events/
│   │   └── bus.rs              # Broadcast event bus
│   ├── storage/
│   │   └── sqlite.rs           # SQLite persistence
│   └── alert/
│       └── notifier.rs         # Console alert formatter
```

### License

MIT

---

<a id="中文"></a>

## 中文

Deribit 交易所 BTC 期权套利实时扫描系统。通过 WebSocket 连接，监控所有 BTC 期权合约，使用多种套利策略扫描定价偏差机会。

### 功能特性

- **实时 WebSocket 数据流** — 自动重连、心跳保活、速率限制
- **7 种套利/信号检测器** — 从无风险结构性套利到统计性 IV 异常
- **终端 UI (TUI)** — 交互式仪表盘，支持筛选、排序和详情查看
- **离线监控模式** — 无需实时连接即可从数据库回顾历史机会
- **SQLite 持久化** — 本地存储所有合约信息、行情和套利机会
- **组合优化器** — 寻找对冲组合以降低保证金、提升年化收益
- **杠杆调整 APY** — 支持可配置杠杆的年化回报计算

### 系统架构

```
┌─────────────┐     ┌──────────────┐     ┌───────────────────┐
│  Deribit WS  │────▶│   事件总线    │────▶│    分析引擎        │
│  (数据流)    │     │  (广播通道)   │     │  (7 种策略)       │
└─────────────┘     └──────┬───────┘     └────────┬──────────┘
                           │                      │
                    ┌──────▼───────┐     ┌────────▼─────────┐
                    │  行情缓存     │     │    套利机会        │
                    │  订单簿       │     │   → SQLite 数据库  │
                    └──────────────┘     │   → TUI 展示       │
                                        │   → 控制台告警      │
                                        └──────────────────┘
```

**模块说明：**

| 模块 | 说明 |
|------|------|
| `ws/` | WebSocket 客户端、身份认证、速率限制器 |
| `market/` | 合约注册表、行情缓存、订单簿、订阅管理 |
| `analysis/` | 7 种策略分析器 + 机会模型 + 组合优化器 |
| `events/` | 广播事件总线 |
| `storage/` | SQLite 持久化层 |
| `alert/` | 控制台通知器 |
| `tui.rs` | 基于 Ratatui 的终端仪表盘 |
| `bin/monitor.rs` | 离线数据库查看器 |

### 套利策略

#### 结构性套利（无风险）

| 策略 | 说明 | 风险 |
|------|------|------|
| **Put-Call Parity（看跌看涨平价）** | 检测 C - P ≠ 1 - K/S 的定价偏差（BTC 结算） | 低 |
| **Box Spread（箱式价差）** | 4 腿纯期权套利，到期保证 USD 收益 = K2 - K1 | 低 |
| **Conversion/Reversal（转换/逆转）** | 合成远期 vs 现货，锁定无风险利润 | 低 |
| **Vertical Arb（垂直套利）** | 行权价排序中的单调性/凸性违反 | 低 |
| **Calendar Arb（日历套利）** | 同行权价远月 < 近月价格（硬约束违反） | 低 |

#### 统计信号（方向性）

| 策略 | 说明 | 风险 |
|------|------|------|
| **Vol Surface Anomaly（波动率曲面异常）** | 通过 Z-score 检测蝶式/配对 IV 异常值 | 中高 |
| **Calendar Spread（日历价差）** | 不同到期日间异常的 IV 期限结构 | 中高 |

### 快速开始

#### 前置条件

- Rust 1.70+（通过 [rustup](https://rustup.rs/) 安装）
- Deribit API 凭证（从 [test.deribit.com](https://test.deribit.com/) 或 [deribit.com](https://www.deribit.com/) 获取）

#### 1. 克隆与配置

```bash
git clone <repo-url>
cd deribit
cp .env.example .env
# 编辑 .env 填入你的 API 凭证
```

#### 2. 编译

```bash
cargo build --release
```

#### 3. 运行实时扫描器

```bash
cargo run --release
```

启动主引擎：连接 Deribit，加载所有 BTC 期权合约，订阅行情流，每 10 秒扫描一次套利机会。机会会打印到控制台并保存到 SQLite。

#### 4. 运行 TUI 监控器（可选）

在另一个终端中：

```bash
cargo run --release --bin monitor
```

打开交互式仪表盘，从数据库读取数据。可与主扫描器同时运行。

### TUI 快捷键

| 按键 | 功能 |
|------|------|
| `q` | 退出 |
| `j` / `↓` | 向下移动 |
| `k` / `↑` | 向上移动 |
| `Enter` | 查看机会详情 |
| `Esc` / `Backspace` | 返回列表 |
| `1`-`9` | 切换筛选器（全部 / 套利 / 信号 / PCP / 价差 / 转换 / 日历 / 波动率 / 组合） |
| `s` | 切换排序方式（利润 / 时间 / APY） |
| `l` | 切换杠杆倍数（1x / 2x / 5x / 10x） |

### 参数配置

所有配置通过环境变量（`.env` 文件）设置：

| 变量 | 说明 | 默认值 | 必填 |
|------|------|--------|------|
| `DERIBIT_CLIENT_ID` | API 客户端 ID | — | 是 |
| `DERIBIT_CLIENT_SECRET` | API 客户端密钥 | — | 是 |
| `DERIBIT_ENV` | `test` 测试网，`prod` 主网 | `test` | 否 |
| `ALERT_THRESHOLD` | PCP 套利阈值（标的价格的比例，如 `0.005` = 0.5%） | `0.005` | 否 |
| `DB_PATH` | SQLite 数据库文件路径 | `deribit.db` | 否 |
| `RUST_LOG` | 日志级别：`trace`, `debug`, `info`, `warn`, `error` | `info` | 否 |

#### 策略参数（硬编码，在 `src/main.rs` 中修改）

| 参数 | 策略 | 默认值 | 说明 |
|------|------|--------|------|
| `alert_threshold` | Put-Call Parity | `0.005` | 最小利润占标的比例 |
| `min_profit_usd` | Box Spread | `10.0` | 最小 USD 利润触发阈值 |
| `min_profit_usd` | Conversion/Reversal | `10.0` | 最小 USD 利润触发阈值 |
| `min_profit_usd` | Vertical Arb | `5.0` | 最小 USD 利润触发阈值 |
| `min_profit_usd` | Calendar Arb | `5.0` | 最小 USD 利润触发阈值 |
| `butterfly_z_threshold` | Vol Surface | `~2.0`（由构造参数 `15.0` 控制） | 蝶式异常 Z-score 阈值 |
| `min_iv_diff` | Calendar Spread | `10.0` | 最小 IV 百分点差异 |

#### 内部常量

| 常量 | 值 | 说明 |
|------|-----|------|
| 心跳间隔 | 30 秒 | WebSocket 保活 |
| 扫描间隔 | 10 秒 | 两次套利扫描之间的间隔 |
| 行情存储频率 | 1/100 | 每 100 条行情存储一次到数据库 |
| 事件总线容量 | 4096 | 广播通道缓冲区大小 |
| 初始延迟 | 30 秒 | 首次套利扫描前的等待时间 |
| 合约刷新周期 | 3600 秒 | 每小时重新加载合约列表 |
| 杠杆选项 | 1x, 2x, 5x, 10x | TUI 杠杆预设 |

### 项目结构

```
deribit/
├── Cargo.toml
├── .env.example
├── src/
│   ├── main.rs                 # 实时扫描器入口
│   ├── lib.rs                  # 模块导出
│   ├── config.rs               # 环境配置加载器
│   ├── tui.rs                  # 终端 UI 仪表盘
│   ├── bin/
│   │   └── monitor.rs          # 离线数据库查看器入口
│   ├── ws/
│   │   ├── client.rs           # WebSocket 管理器 & RPC 客户端
│   │   ├── auth.rs             # HMAC-SHA256 身份认证
│   │   └── rate_limiter.rs     # API 速率限制
│   ├── market/
│   │   ├── instruments.rs      # 合约注册表 & 解析
│   │   ├── ticker.rs           # 实时行情缓存
│   │   ├── orderbook.rs        # 订单簿管理器
│   │   ├── subscriber.rs       # 频道订阅辅助
│   │   └── trades.rs           # 交易数据类型
│   ├── analysis/
│   │   ├── opportunity.rs      # 机会 & 交易腿模型
│   │   ├── portfolio.rs        # 组合优化器
│   │   ├── put_call_parity.rs  # 看跌看涨平价套利
│   │   ├── box_spread.rs       # 箱式价差套利
│   │   ├── conversion.rs       # 转换/逆转套利
│   │   ├── vertical_arb.rs     # 垂直价差套利
│   │   ├── calendar_arb.rs     # 日历套利（硬约束）
│   │   ├── vol_surface.rs      # IV 曲面异常检测器
│   │   └── calendar_spread.rs  # 日历价差（IV 信号）
│   ├── events/
│   │   └── bus.rs              # 广播事件总线
│   ├── storage/
│   │   └── sqlite.rs           # SQLite 持久化
│   └── alert/
│       └── notifier.rs         # 控制台告警格式化
```

### 许可证

MIT
