# 系统架构

CodeBeat 由桌面主程序、独立后台执行器和共享业务库三层组成：`desktop/src-tauri` 只管理 Tauri 窗口、托盘、通知与 IPC；`crates/codebeat` 承载领域模型、持久化、任务执行与 Windows 原生集成，不依赖 Tauri；`crates/codebeat-runner` 与主程序装在同一目录，只做 CLI 参数分发。

本文合并自早期 `ARCHITECTURE.md`、`PROJECT_STRUCTURE.md` 与 `WINDOWS_NATIVE.md`。历史上另有一份结构调整建议（`PROJECT_STRUCTURE_PROPOSAL.md`），其目标结构已经落地，故不再单独保留。

## 目录

- [工程与状态约束](#工程与状态约束)
- [项目结构](#项目结构)
- [Windows 原生集成](#windows-原生集成)

## 工程与状态约束

完整目录与依赖关系见 [当前项目结构](#项目结构)。桌面主程序 `desktop/src-tauri` 只管理 Tauri 窗口、托盘、通知展示和 IPC；Vue 前端按 components/pages/services/stores/composables/types 分组。

共享库 `crates/codebeat`（Cargo 包名 `codebeat-lib`）无 Tauri 依赖。`models` 定义序列化类型，`tasks` 负责目录校验、UTC/本地时区一致性、错过策略、任务服务及执行。`db` 使用 rusqlite bundled SQLite、WAL、10 秒 busy timeout、user_version=3 迁移（包含预启动执行记录及持久化通知收件箱）。数据库结构将完整任务请求保存为版本化 schema 下的 JSON，运行结果及心跳独立列；与技术规格中的建议 SQL 在存储形态上不同，领域字段保持齐全。

共享库的 `providers` 模块 唯一负责官方 CLI、JSON-RPC、额度与会话解析。Codex 支持多 bucket 与 primary / secondary，不预设时长；Claude 不提供未经验证的额度值。RPC 每连接握手、递增 ID、stdout 与 stderr 分流、25 秒单请求超时、进程退出检测；每次查询重新连接，失败不自动重发模型请求。

共享库的 `platform` 模块通过 Rust 原生 API 调用 Task Scheduler COM、当前用户注册表和 ACL；职责分别放在 `scheduler.rs`、`autostart.rs`、`security.rs`、`packages.rs` 和 `diagnostics.rs`。独立通知由 Rust/Tauri 辅助进程承载 Vue 卡片，任务由 Windows 调度服务驱动。具体实现见 [Windows 原生集成](#windows-原生集成)。

`codebeat-runner` 与 GUI 同一安装目录，只负责 CLI 参数与命令分发；执行与预启动逻辑分别位于共享库的 `tasks/execution.rs` 和 `pretrigger/worker.rs`。每次任务执行先 `BEGIN IMMEDIATE`，检查全局暂停、全局执行上限 1，原子从 scheduled 变为 claimed 并插入唯一 attempt。未获得抢占不启动 Provider。运行时 Windows Job Object 设置 KILL_ON_JOB_CLOSE，执行器异常退出也会终止所属进程树。每 10 秒心跳，3 分钟失联后只标 needs_review。取消时先尝试终止进程树，再强制结束；30 分钟超时标 needs_review，不扩大权限，也不将未知超时误称为已确认的审批请求。操作系统可能已提交的模型工作不保证可撤销。

结构化成功必须有对应 turn.completed / result success 且进程返回 0。仅返回 0 记 completed_unverified。失败 / 审批事件优先于成功。通知失败不回滚执行结果，通知去重记录失败时撤销，供后续可用路径重新发送。任务结果通知目前由完成的 Runner 发出；进程崩溃后的 needs_review 会在 UI 中呈现。

任务创建顺序：DB pending_registration → COM 注册 → scheduled，注册失败持久化 registration_failed；编辑先隔离状态，再移除旧入口、更新请求并注册；取消运行中请求变为 cancelling，执行器心跳处理。再次执行失败任务需要用户编辑并确认，没有自动重试已提交请求。

Codex 会话使用 `providers/session_lock.rs` 检查实际系统文件锁，列表快照与执行前检查分开。任务请求保存 `locked_session_behavior`（fork/pause）；执行时占用则运行 `codex exec fork` 或以 `SESSION_IN_USE` 暂停，无法确认占用状态时以 `SESSION_LOCK_UNKNOWN` 暂停。分叉策略不修改请求中的源会话 ID，执行日志记录实际 CLI 事件。新任务表单不再提供高级设置，使用 `full_access`、通知和唤醒开启、错过时询问的固定配置；旧权限档位仍可读取。

维护入口需用户在设置中启用，当前用户登录触发 + 每 5 分钟重复，不唤醒设备。依据最新官方窗口做提前提醒；重置后只有后续窗口推进且百分比下降才确认恢复，否则提醒尚未确认。每日 Coding 额度调配使用独立每分钟检查入口，默认开启；一次性唤醒计划按下一触发节点安排。

生产前端不提供内置假数据。测试 fixture 是单独可执行文件，只有测试使用，不包含在安装包中。Runner 集成测试在隔离目录将 fixture 作为临时 PATH 中的原生 CLI，验证无 GUI、重复进程触发、安全 stdin 提示词和最终状态。浏览器测试加载实际 dist 资源，并通过独立 IPC fixture 提供测试额度与会话，不访问真实模型接口。

可用性限界：后台恢复要求用户已登录且官方 CLI 环境可访问；不运行在 SYSTEM，不保存 Windows 密码。关机不能开机。DPAPI、完整英文表单、运行中事件推送、升级迁移回滚向导、真实账号完整预启动周期与干净系统验收尚未完成。当前版本 user_version 大于 3 会拒绝打开；备份应在应用与 Runner 全部停止后复制整个应用数据目录（包括 SQLite WAL/SHM）。

## 项目结构

GitHub 根目录保留项目说明、Rust 工作区、CI 和几个职责清晰的目录。桌面应用相关内容集中放在 `desktop/`。

```text
codebeat/
├─ desktop/                       # 桌面应用与前端工具链
│  ├─ src/                        # Vue 页面、组件、服务、状态和类型
│  ├─ src-tauri/                  # Tauri 主程序、原生窗口、托盘与 IPC
│  │  ├─ src/
│  │  ├─ capabilities/
│  │  ├─ icons/
│  │  ├─ installer/
│  │  ├─ Cargo.toml
│  │  └─ tauri.conf.json
│  ├─ assets/                     # 字体、品牌源文件和 logo
│  ├─ tests/ui/                   # 页面测试和隔离 IPC 模拟数据
│  ├─ index.html
│  ├─ package.json
│  ├─ package-lock.json
│  ├─ playwright.config.ts
│  ├─ tsconfig.json
│  └─ vite.config.ts
├─ crates/
│  ├─ codebeat/                   # 共享库，Cargo 包名 codebeat-lib
│  │  ├─ src/
│  │  │  ├─ db/                   # SQLite、状态和消息持久化
│  │  │  ├─ models/
│  │  │  ├─ tasks/
│  │  │  ├─ providers/
│  │  │  ├─ usage/
│  │  │  ├─ pretrigger/
│  │  │  ├─ notifications/
│  │  │  └─ platform/
│  │  ├─ upgrades/                # SQL 升级脚本
│  │  └─ tests/                   # Rust 测试及协议/额度测试输入
│  └─ codebeat-runner/            # 独立后台任务执行器
├─ docs/                          # 文档及设计参考图
├─ scripts/                       # 跨前端/后台的构建、打包及验证
├─ .github/workflows/             # Windows CI
├─ Cargo.toml                     # Rust workspace 定义
├─ Cargo.lock
├─ .gitignore
├─ README.md
├─ CHANGELOG.md
├─ TECHNICAL_SPEC.md
└─ LICENSE
```

### 依赖关系

```text
Vue → Tauri IPC → desktop/src-tauri（codebeat）
                              ↓
                      crates/codebeat（codebeat-lib）
                              ↑
                  crates/codebeat-runner（codebeat-runner）
```

共享业务库不依赖 Tauri。Runner 不加载桌面界面，两者复用任务规则、执行器、数据库及 Windows 适配。

### 从根目录运行

```powershell
npm --prefix desktop ci
cargo build --locked -p codebeat-runner
New-Item -ItemType Directory -Force desktop/src-tauri/binaries
Copy-Item target/debug/codebeat-runner.exe desktop/src-tauri/binaries/codebeat-runner.exe
npm --prefix desktop run desktop
```

```powershell
./scripts/verify.ps1
npm --prefix desktop run test:ui
./scripts/package.ps1
```

也可以先 `cd desktop`，再使用 `npm ci`、`npm run desktop`、`npm test`、`npm run test:ui` 等命令。

### 资源和生成文件

- 前端字体及图标都在 `desktop/` 内；独立通知进程复用嵌入前端的 WOFF2，不需要外部 TTF 目录。
- 原始 logo SVG 保留在 `docs/images/logo.svg`，位图在 `desktop/assets/brand/logo.png`，品牌清单按仓库根目录记录路径。
- Rust 额度测试数据归入 `crates/codebeat/tests/fixtures/`，不依赖前端目录。
- 前端构建在 `desktop/dist/`；Rust 与安装包仍在根目录 `target/`。
- 生成的日志和截图仍在 `docs/logs/`、`docs/screenshots/`；Playwright 报告在 `desktop/playwright-report/`，均不提交 Git。
- 数据目录、系统任务、自动启动路径、应用标识不因源代码目录重排而改变。
- 此前开发记录中出现的根目录 `src/`、`src-tauri/`、`assets/` 和前端配置路径，迁移后均以 `desktop/` 为前缀。

### 迁移验证

45 项前端单元测试、66 项 Rust 测试、88 项页面测试通过；格式、Clippy、干净源码目录构建、字体校验、原生通知及真实托盘窗口测试通过。NSIS 安装包已生成并核对内容与图标。前端源码、锁文件和数据库升级脚本共 69 个文件与迁移前备份逐字节相同；23 个品牌资源哈希未变。详情见本地生成的 验证记录。未暂存、提交或推送 Git，未使用真实重置卡。

## Windows 原生集成

应用运行时及卸载清理已移除 PowerShell 依赖。开发构建、验证和工作区路径迁移仍可使用 `scripts/` 下的 PowerShell 脚本。Windows 系统任务名称、自启注册表值、数据目录和数据库统一使用 `codebeat` 标识，不保留旧品牌兼容逻辑。

“每日Coding 额度调配”默认开启，采用智能均衡、自动轻量触发和允许唤醒。工作结束前显示当前工作日，结束后展示下一生效工作日，夜班按班次开始日期归属。当天首次确认的使用或自动触发确定基准，正常展示节点每五小时排列，执行节点每轮累加一分钟，例如展示 07:10／12:10／17:10，执行 07:10／12:11／17:12。官方提前重置会截断旧展示周期，并从已观测到的重置时间重新排列五小时刻度；后续使用观测时间只放在详情中。`display_times`、`slots.execute_at` 与 `day_start` 保存基础展示、实际执行与起点；前端结合历史窗口的 `closed_at` 投影额度周期，执行前仍以官方真实窗口检查为准。页面不再提供模式选择、高级折叠项和执行记录入口。

数据库版本 6 一次性启用旧开发配置中的调配默认项，保留工作时间与提供商；已记录的消耗异常、执行失败或窗口未确认暂停不会自动解除。之后用户关闭总开关会正常持久化。首次创建数据库时保存默认设置，桌面启动时同步后台检查与唤醒注册，保证默认开启能够实际运行。

额度读取不等于触发。计划和发送前的两次检查共用 `check_window`，`automatic_allowed` 实际参与 `due_for_plan`。0% 加未来浮动恢复时间需要两次独立读取证明恢复时间随查询移动，才确认空闲；固定的未来恢复时间仍不授权发送。任意正用量快照会先阻止额外请求，但需要相隔至少 30 秒、同一账号和额度池、恢复时间稳定在两秒内的独立快照，才进入窗口历史。App 成功请求使用真实发送记录作为证据；官方仍四舍五入显示 0% 时保留五小时加一分钟的防重复间隔，不因这一个 0% 永久关闭功能。读取失败或异常消耗仍会暂停。界面区分“自动触发”和“检测到使用”；原始额度快照保留。

迁移 008 清除可重建的旧运行缓存，停用提供商的缓存不会被后台刷新重新创建。共用时间尺覆盖工作时段与完整额度展示周期，工作和午休刻度在上方，Codex 周期节点及末端在下方，所有轨道使用同一比例。展示不累加执行延迟；各色带只在自身内部边界画细分割线，两端不画线，官方提前重置点以橙色标记。标签保持单行，拥挤时左右避让；原始使用与恢复时间保留在详情中。每次 tick、未到点、额度未确认、领取被拒和完成状态写入 `logs/pretrigger-decisions.jsonl`，包含时间、进程、执行文件时间戳和决策依据，轮换保留上一份。`codebeat-runner pretrigger-status` 根据本地缓存输出计划、已核实窗口和下次唤醒，不请求服务商。打包记录主程序和 runner 的 SHA256，用于核对计划任务运行的版本。

额度恢复通知统一由 `Database::cache_usage` 处理。迁移 007 持久化通知基线和重置卡兑换记录；基线、快照、消息及去重记录在同一写事务中提交，桌面轮询和 runner 共用。失败读取、过期观测和账号切换不会生成恢复通知。官方额度下降或已确认的新窗口会提示；提前恢复与正常到期使用不同标题。App 内兑换收到 `reset` 或 `alreadyRedeemed` 后按幂等键通知，后续刷新不再重复报为官方重置。其他地方的兑换只能推断：权威卡片计数减少、额度恢复、明细完整且消失卡片尚未到期时才归为卡片重置；明细截断、到期或缺失时发送来源未确认的恢复消息。卡片过期本身不生成兑换成功消息，缺少 expiresAt 与明确为 null（不失效）分别记录。测试只使用隔离数据库和协议夹具，不消费真实卡片。

自动轻量触发在已知官方重置时间之后至少等待 60 秒，保留官方时间的秒数；唤醒计划同步使用延后的候选时间。执行前重新读取额度，仍未恢复、无法确认或正在使用时不发送请求。

### 实现

- `crates/codebeat/src/platform/autostart.rs`：当前用户注册表自启及隔离注册表测试。
- `platform/scheduler.rs`：Task Scheduler COM、任务注册和删除、任务清理及调度策略测试。
- `platform/security.rs`：当前用户数据目录 DACL、句柄释放及 SID/继承检查。
- `platform/packages.rs`：当前用户 MSIX 包发现、版本排序及安装目录查询。
- `platform/diagnostics.rs`：组合任务和电源诊断；直接调用系统目录中的 `powercfg.exe`，负责超时、输出长度限制和代码页解码。
- `platform/mod.rs`：保留业务调用接口，协调任务清理、自启和通知投递。底层 Win32/COM/WinRT 使用 `windows` 0.61.3；Job Object 继续使用 `windows-sys`。各模块的测试随实现放置，运行时无需 PowerShell，也不自动提权。
- `desktop/src-tauri/src/notification_helper.rs`：`codebeat.exe --notification-helper` 从 stdin 接收 JSON，运行独立通知窗口。此模式在单实例插件和正常桌面初始化之前分流，不建立托盘、不运行额度刷新、不执行数据库恢复或模型请求。
- `desktop/src/components/NativeNotification.vue`：通知卡片及随包字体。标题和正文通过 Vue 文本插值显示，不解析 HTML、XAML 或 shell。
- `codebeat-runner cleanup-registrations`：在打开数据库前清理当前用户系统任务及自启，供 NSIS 卸载器调用。失败会停止卸载，避免留下指向已删除程序的系统任务。

### 保留行为

调度保留交互式用户身份、最低权限、一次性触发、登录触发、每日重复、每分钟预触发、每五分钟维护、补跑、电池运行和忽略重叠实例。周期预触发不会唤醒机器；优化器唤醒为一次性任务。COM apartment 在调用线程内初始化和释放，已有 STA 的线程继续使用原 apartment。

目录权限关闭继承，仅授予当前用户完全控制，并使子目录和文件继承。自启命令保留带双引号的程序路径和 `--background`。

通知保持 500×176 逻辑像素卡片及外围阴影，不抢焦点、不显示任务栏按钮、右上角排列。主程序退出后仍可独立显示。跨进程命名互斥量管理位置；溢出通知排队，等待期间可以响应 `.dismiss`。未交互时不自动关闭；鼠标移入/移动或键盘聚焦后，原生端确认窗口可见且卡片位于工作区，写入 `.read`，从首次确认起两秒后关闭。计时和外部关闭检测由 Rust 执行，不依赖 WebView 定时器。点击“查看”通过单独的进程参数打开已验证的任务 UUID 或 tasks/usage 页面。

通知启动要求已安装的 WebView2，与桌面程序一致；不再需要 WPF/PowerShell。通知未写入 Windows 通知中心，也未新增系统勿扰设置集成。

### 验证

```powershell
./scripts/verify.ps1
# 仅注册隔离测试任务，不执行任务或调用模型。
./scripts/test-pretrigger.ps1
# 嵌入前端，测试真实独立 WebView2 进程。
npm --prefix desktop run build
cargo build --locked -p codebeat --features tauri/custom-protocol
./scripts/test-notification.ps1
```

常规 Rust 测试包含隔离注册表键的自启往返、DACL/SID/继承检查。显式调度测试检查四类任务注册后的 XML、公共注册接口和电源诊断，并清理临时任务。通知脚本使用临时数据目录、隔离 WebView2 profile 和唯一确认文件，通过仅本机的 DevTools 连接验证实际渲染、READY、文本、已读计时、排队和关闭；不会启动正常桌面模式或发送模型请求。截图保存在 `docs/screenshots/notifications/rust-helper-*.png`。

全屏游戏、多显示器不同 DPI、锁屏及物理睡眠唤醒仍需 Windows 人工回归。Rust 原生调用仍依赖 Windows 服务和当前用户权限；不支持的平台继续返回 `UNSUPPORTED_PLATFORM`。
