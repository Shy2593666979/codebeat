# 交付与验收记录

按阶段记录实际改动、验证方式与未完成事项，另附品牌改名说明、Windows 人工验收矩阵与说明图的生成提示词。文中链接指向本地生成或 CI 上传的截图与日志，不纳入版本库。

## 目录

- [分阶段交付记录](#分阶段交付记录)
- [品牌改名记录](#品牌改名记录)
- [Windows 人工验收矩阵](#windows-人工验收矩阵)
- [README 说明图生成提示词](#readme-说明图生成提示词)

## 分阶段交付记录

### 蓝色 zzz Logo（2026-10-17）

使用最新上传的 logo 源图替换前一版黑色标志。保留源图自带的浅色面板、蓝色 zzz 图案和外部透明区域，不添加额外底板、边框或阴影。同步更新标题栏、托盘面板、系统托盘、窗口/任务栏、EXE、安装与卸载图标，以及浏览器 favicon。

类型检查、生产构建与 29 项浏览器回归通过，首页和托盘实际截图已检查；源 PNG 完整嵌入 SVG，图案没有重绘。见首页效果、托盘效果、透明区域检查、浏览器回归、打包日志和Windows 实际图标资源。

### 项目目录与请求默认可用（2026-10-17）

按用户要求移除任务弹窗中的工作目录说明、项目范围提示和请求授权勾选；不再要求先去设置授权目录。新建、编辑、后台 Runner、托盘立即继续及打开官方工具统一取消目录白名单和 consent 检查；旧 consent 字段保留兼容，缺省为 true。仍按所选会话的真实 cwd 执行，校验目录存在与会话一致性。

新建会话可直接输入任意实际可访问的项目目录。设置中的原目录列表改为“常用项目目录”，仅作为快捷选择，空列表或其他常用目录都不限制当前项目。原始规范对应条目已按本次要求更新。

新增/更新回归覆盖：空目录列表、项目不在常用列表、旧 consent=false、缺省 consent、无授权的独立 Runner、桌面路径直接打开工具、无复选框的任务保存；测试通过伪 CLI 执行，没有发送真实模型请求。41 项 Rust 测试、22 项前端测试、29 项浏览器测试及 Clippy 通过。见后台回归、浏览器回归、前端测试、打包及产物校验。

### Logo 使用透明背景（2026-10-17）

按要求去掉附加的白色圆角底和边框，保留原始黑色 CODE / zzz 图案与透明背景，重新生成全部界面 PNG、Windows ICO、安装与卸载图标。原始上传图片未改动。

类型检查、生产构建与 29 项浏览器回归通过；PNG 像素检查确认存在透明区域且没有不透明白底，见像素检查。EXE 与安装程序的实际图标资源见资源核对，另见打包日志、首页效果及托盘效果。下方保留上一轮带底色版本的历史记录。

### 新版 Logo（2026-10-17）

使用新增的 CODE / zzz 黑色标志替换应用标题栏、托盘面板、系统托盘、窗口/任务栏、EXE 与安装包图标，并同步浏览器 favicon。原始 PNG 嵌入 SVG 包装，保持图案不变，调整透明留白并加白色圆角底以适配深色任务栏；使用 Tauri CLI 生成多尺寸 Windows 图标，ICO 包含 16/24/32/48/64/256 像素。

类型检查、生产构建和 29 项浏览器回归通过，首页与托盘截图已检查；本轮未修改 Rust 业务逻辑。补齐构建脚本的图标目录变更检测，避免缓存复用旧图标，并显式配置 NSIS 安装与卸载图标。品牌源文件与生成资产哈希见[品牌清单](../desktop/assets/brand/manifest.json)，EXE 和安装程序的实际 Windows 图标资源见资源核对。另见图标生成、浏览器测试、打包日志、首页效果及托盘效果。

### 托盘整体缩小（2026-10-17）

托盘主卡片和浮层统一缩小 15%，字体、图标、间距、圆角和尖角同步调整。主卡片宽度由 326 减至约 277 逻辑像素；独立透明窗口从 606 × 620 减至约 515 × 527。同步调整 Rust 定位的物理像素换算和窗口配置，保留边缘换向、菜单行为和任务执行校验。

类型检查与生产构建、29 项浏览器回归、2 项桌面 Rust 测试及 Clippy 通过；其余 Rust 模块未修改。见缩小后效果、浏览器日志、Rust 日志、打包日志及产物校验。

### 托盘展示页（2026-10-17）

按提供的托盘图片重做独立透明弹出面板，去除快捷键列并将主菜单收窄到 326 逻辑像素。保留菜单分组、品牌与运行状态、底部尖角、右侧浮层及真实工具图标；接入任务、额度、设置、暂停、继续和退出，补齐边缘换向与键盘操作。41 项 Rust、22 项前端和 29 项浏览器测试通过，Clippy 无警告。说明、截图及日志见[托盘交付说明](FEATURES.md#托盘展示页)。

### 默认窗口与精简首页（2026-10-17）

主窗口默认从 1348 × 1024 改为 1200 × 820 逻辑像素，居中打开，保留缩放和最大化，最小尺寸仍为 1024 × 700。首页移除定时任务列表，保留顶部新建入口与任务页完整功能；今日状态和 Token 曲线紧接额度卡片，双列对齐并增高，曲线绘图区由 120 增至 210 像素。

类型检查、生产构建、22 项前端测试与 26 项浏览器测试通过。新增从 Tauri 配置读取默认窗口尺寸的实际浏览器回归，确认首页无滚动即可显示四张卡片、任务页仍列出全部任务、首页仍可新建任务，并检查最小窗口和深色模式。见默认效果、深色效果、最小窗口、浏览器回归、构建、打包及产物校验。

### 单箭头刷新与通知倒计时（2026-10-17）

7 个 Vue 文件中的刷新、重新检测图标统一为单箭头 RotateCw，原操作与加载旋转保留。应用内通知和 Windows 独立通知底部增加细进度条，首次判定已看到后开始缩短，2 秒后自动关闭；重复悬停不延长，未看到时不显示倒计时且持续保留，不显示已读/未读文字。

类型检查与生产构建通过，25 项浏览器回归通过；Windows WPF 实测验证未看到时保留、进度条中途缩短及首次确认后 2 秒关闭。见浏览器日志、原生通知测试、应用内截图、独立通知截图、打包日志及产物校验。

### 页面切换布局稳定（2026-10-17）

移除各页单独覆盖侧边栏宽度、标题栏高度和内容内边距的规则，五个页面共享同一套响应式外壳。工作区固定滚动条占位，关闭滚动锚定；路由更新后在渲染周期内直接重置内容滚动位置。

22 项前端测试和 25 项浏览器测试通过。新增 1348、1100、620 像素宽度下的五页往返导航回归，比较侧栏、导航、标题栏、内容起点和可用宽度，覆盖长页滚动及滚动条出现与消失。见构建日志、单元测试、浏览器回归和截图。Rust 源码未改，沿用此前 40 项测试结果。安装包与普通桌面启动见打包日志、产物校验及启动记录。

### Token 曲线与卡片简化（2026-10-17）

Token 柱状图改为单条平滑曲线；“全部工具”为每天合计，单选工具为对应每日用量。移除卡片总量、输入/输出汇总、颜色图例和底部说明。首页与额度页共用，点击日期仍可查看明细；额度页同时支持工具和天数筛选。

22 项前端测试与 22 项浏览器回归通过，浅色/深色实际截图已检查。Rust 用量统计未改，沿用前阶段 40 项测试结果。见[曲线效果与日志](FEATURES.md#每日-token-消耗监控)，最终安装包大小和 SHA-256 以产物校验为准。

### 每日 Token 消耗监控（2026-10-17）

首页趋势改为最近 7 天的本机 Token 消耗，支持 Codex / Claude Code 筛选、日期和整周明细；额度页支持 7/14/30 天。Rust 后台增量读取本地日志，按本地日期统计输入、输出及缓存子集，累计与分段记录去重；缺失和部分数据明确标记。原剩余额度卡片保留。

40 项 Rust、21 项前端及 22 项浏览器测试通过，真实本机日志只读探测通过；部分 Codex 日志不兼容/过大，聚合结果已标为 partial。详见[Token 用量说明与执行日志](FEATURES.md#每日-token-消耗监控)。本轮重新编译桌面程序与 NSIS，最新大小及哈希见产物校验。

### 完成统一下拉框（2026-10-16）

接手未完成的 Select.vue，修复 ref 模板访问与数值类型错误；使用泛型原样支持字符串、数字、0 与空字符串，不将设置数值转换成字符串。已统一 5 个文件中的 13 处下拉框，源码不再使用原生 select。补齐搜索、禁用项跳过、焦点与键盘操作、弹窗 Escape 优先级、视口定位和深色样式。

18 项前端测试和 19 项浏览器测试通过，类型检查与生产构建通过。本轮重新编译桌面与 NSIS，最终资源与哈希以产物校验为准。实现边界、截图与日志见[下拉框交付说明](FEATURES.md#统一下拉框交付说明)。下方保留历史阶段记录。

### 统一弹窗与真实工具图标（2026-10-16）

重新设计新建/编辑任务、立即继续、确认排程、新建会话、删除、恢复默认、连接诊断和运行日志弹窗；统一浅蓝色标题、分区、圆角和固定底部操作，支持深色与小窗口、Escape、Tab 循环及焦点返回。任务表单移除嵌套 form，会话搜索不会意外提交；切换提供方会清空会话与授权勾选。

Codex 全局使用黑白 OpenAI 图形，Claude Code 使用橙色 Claude 星芒，均为本地 SVG。字体文件与上一轮一致。11 项前端测试、16 项浏览器测试通过，本轮重新编译桌面及 NSIS。截图与流程说明见[弹窗检查](FEATURES.md#弹窗与工具图标调整)，实际产物大小、SHA-256 与资源一致性见最新解包校验。下方保留历史阶段记录。

### 保持现有字体并缩小安装包（2026-10-16）

网页四份 WOFF2 和原生实际使用的 Regular / SemiBold TTF 均保持原文件及完整字符覆盖。移除原生通知未使用的 Medium / Bold 打包资源，NSIS LZMA 字典增至 64 MiB；安装包从 **40.54 MiB 降至 30.04 MiB（减少 25.91%）**。新包为 31,495,493 字节，SHA-256：`49a212d5aef3ab7a8d0c5998b396b36310c08441cf81970a1899723d5c6910b5`。

逐像素及字体哈希检查、解包校验、使用解包字体的原生通知测试及桌面启动通过。本轮重新执行前端生产构建、Rust Release 和 NSIS 打包；业务源代码未变，沿用前阶段单元/浏览器回归结果。详见[优化说明](FEATURES.md#字体体积优化)。下方尺寸为各阶段历史产物。

### 内置字体与排版校准（2026-10-16）

采用 Noto Sans SC 派生的 CodeBeat UI，随软件内置完整字符覆盖的 400/500/600/700 四个字重；页面使用 WOFF2，原生通知通过私有文件 URI 加载相同字形的 TTF，无需安装系统字体。已统一标题/正文/操作文字的字重，调大设置说明文字并校准行高与标题字距。基于候选对照选择近似字体，不声称它就是参考图原字体。

33 项 Rust、11 项前端、12 项浏览器测试通过，Clippy 通过；原生通知同时验证四个字重的实际文件 URI 及原有两秒关闭逻辑。浏览器检查实际绘制字体为打包的 CodeBeat UI，而非只检查 CSS 声明。见 字体对照、字体诊断、原生测试、Rust、浏览器、打包。字体许可与哈希随安装包提供。

最终安装包 42,509,297 字节（40.54 MiB），因保留完整中文字体而增大；解包检查确认四个字体、许可及来源清单均与构建文件一致。普通启动响应正常，启动 stderr 为空。SHA-256：`db67fed6ba3c44af188984f979b4ee2758dbb7eaefc5a9a31699f478fea86d22`。

### 隐藏已读状态文字（2026-10-16）

应用内及独立 Windows 弹窗均移除“已读 / 未读”文字，仅在内部记录状态；持续保留和交互确认后两秒关闭的规则不变。11 项浏览器测试及原生 WPF 计时测试通过，且验证界面不含状态文字。日志：前端构建、浏览器、原生、打包。

安装包已更新为 5,662,619 字节，解包核验及桌面启动通过，启动 stderr 为空。SHA-256：`a9f43a544373ccdcab9c16fb241cea52156fb38fd23b5174d260113026754400`。

### 消息已读规则（2026-10-16）

已取消应用内及原生通知的固定超时。未读持续保留，可见状态下鼠标移入或键盘聚焦视为交互确认，首次确认后两秒关闭；未展示的消息持续排队。应用内额外检查前台焦点、文档可见、卡片完整可见及遮挡。具体定义和日志见 [通知说明](FEATURES.md#右上角消息通知)。**11 项浏览器回归及原生 WPF 已读/计时测试通过**。

33 项 Rust 测试、11 项前端测试、格式及 Clippy 检查通过，见 本轮常规验证。最终安装包 5,661,607 字节，解包核验通过；桌面启动响应正常、stderr 为空。SHA-256：`29a75f7b45ea46219e2eb449e20678049d3eee7204985a4c083cb6ae8b157156`。

### 补齐操作反馈弹窗（2026-10-16）

“已恢复该工具的后续调度”等操作反馈及全局错误现已统一为应用右上角卡片，移除原页内消息横条。前端构建及 **10 项浏览器测试通过**，新增回归覆盖这条具体消息、重复切换、自动关闭/悬停保留、关闭按钮和安全显示错误文本。见 实际截图、回归日志、打包日志。后端源码本轮未变，沿用前轮 33 项 Rust 测试与原生通知验证结果。

最终安装包 5,659,292 字节，解包核验通过；普通桌面启动响应正常，启动 stderr 为空。最新 SHA-256：`93cf3d9c77c9127f4405292ffb24e3a80b6b893abaeee44e662bda57e05a0435`。

### 右上角消息通知（2026-10-16）

已按新增消息通知图实现独立 WPF 通知卡片，替换系统 Toast，包含任务/额度跳转、关闭、自动收起、声音开关和多条消息排队。实际截图、行为与边界见 [通知说明](FEATURES.md#右上角消息通知)。常规验证为 **33 项 Rust、11 项前端、8 项浏览器测试通过**；另行显式执行的原生通知进程测试及 WPF 布局/按钮脚本通过。普通 Rust 测试中现有三项环境测试为 ignored。浏览器回归包含提供方检测持续等待时仍能立即定位通知目标任务。

本轮日志：常规验证、浏览器、原生进程、最终打包。

本轮最终安装包 **5,658,072 字节**，SHA-256 `f6c814c8bc0fd9bc2004c6df91d9c476b024ab36224efaf5510d1b8dcf01b877`。解包核验通过；通过 `--open-page usage` 常规启动后进程响应正常，启动记录及空的 stderr已保存。未执行干净系统安装。

### 五图界面重做（2026-10-16）

已根据 `docs/screenshots/reference` 的五页参考图重写首页、任务、会话、额度和设置，接入真实 IPC；加入会话内容读取、额度历史、设置导入导出、通知细分、提供方暂停和周期任务。详见 [视觉核对及功能对应表](FEATURES.md#五张参考图对应实现与核对记录) 和 五页对照。本轮仍有字体、背景、图标及部分尺寸差异，没有宣称达到 100% 还原。

本轮常规验证为 **32 项 Rust 测试、11 项前端测试、6 项浏览器交互测试通过**；Rustfmt 与 Clippy `-D warnings` 通过。两项需机器环境的测试仍为 ignored。执行证据：完整验证、浏览器测试、最终打包。下方原阶段记录保留作为早期交付历史，本轮记录与最终产物校验优先。

新版 NSIS 安装包已生成（5,635,186 字节），解包核验通过，主程序及 Runner 与构建产物相符；新版常规启动进程响应正常、窗口标题为 codebeat，启动 stderr为空。未执行安装向导，因此不把解包或启动检查视为干净系统安装验收。

本记录对应 2026-10-16 在本地 Windows x64 的真实实现、编译与测试。代码已创建，包含可构建的桌面程序、独立 Runner、测试、CI 和 NSIS 配置；不是静态页面原型。**工程预览版，规格中的正式 Release Acceptance Checklist 尚未全部通过。**

| 阶段 | 已交付代码 | 验证与边界 |
|---|---|---|
| A 骨架与视觉 | Vue3 / TS / Vite、Tauri 2、Rust workspace；参考素材、五页导航、四主题 / 深色、托盘、单实例、隐藏窗口 | TS / Vite 构建通过；8 项前端测试、2 项浏览器交互测试通过；开发服务器 HTTP 200；主程序编译并常规启动，进程响应正常。完整英文表单、首启向导、桌面全部交互验收待完成 |
| B 数据 / 检测 | rusqlite / WAL / v1 migration、CLI 原生路径 / 版本 / 官方登录检测、目录 ACL | Rust 测试通过；本机两工具均检测为 ready / authenticated。四种安装组合仍需独立环境回归 |
| C 额度 | Codex JSON-RPC 握手、多 bucket 窗口、真实时间与来源、未知数据表达 | 本机 Codex 查询返回 2 个真实窗口；fixture 覆盖缺失 / 异常百分比、周窗口、秒 / 毫秒；Claude 为 unavailable，无私有 API |
| D 会话 | Codex thread/list 分页搜索、thread/read 验证、Claude JSONL best-effort 只读扫描 | 两工具本机均成功查询 5 条分页记录并返回后续游标；中文 / BOM / 损坏 JSONL / 无效游标测试通过；1 万条 / 项目搬迁 / 全版本兼容待回归 |
| E 调度 / 执行 | 每任务 COM 入口、WakeToRun、错过策略、独立 Runner、原子抢占、心跳、Job Object、受限参数 / stdin prompt | SQLite 多连接 / 多进程抢占、独立 Runner 多进程触发、四类结果、Job Object 终止测试通过；Windows 当前用户注册 / 查询 / 删除实测通过。未发送真实模型任务，未做实体机睡眠 / 重启测试 |
| F 通知 / 历史 | WinRT Toast 调用、持久化去重、官方窗口提醒、维护入口、日志 / 历史查看、轮转、取消 | 事件及去重持久化测试通过，完整系统 Toast 展示与电源周期仍待人工验证。已增加按作息预启动、后台独立检查与真实请求结果确认，见 [实现说明](FEATURES.md#按作息预启动额度窗口)；真实账号完整周期仍待验证。Runner 进度通过 DB 轮询呈现，实时跨进程事件推送待完善 |
| G 发布 | Windows Actions、锁文件、NSIS currentUser、WebView2 下载引导、主程序 / Runner 同目录、隐私 / 许可文件、SHA256SUMS | 本地 Release 和 NSIS 已实际生成；7-Zip 解包确认包含两 EXE 和声明文件。未推送 GitHub，不能声称远程 CI green；Win10/11 干净安装 / 升级 / 卸载与签名未验收 |

### 运行和安装

- 安装包：`target/release/bundle/nsis/codebeat-Setup-x64.exe`
- 校验：同目录 `SHA256SUMS`
- 主程序：`target/release/codebeat.exe`
- 独立执行器：`target/release/codebeat-runner.exe`
- 从源码运行：见 [README](../README.md)。

最终产物大小与 SHA-256 以安装包旁的实际文件为准，不使用目标体积冒充结果。没有修改用户项目，也没有向模型发送测试提示词；真实 CLI 探测仅查询健康、额度和会话元数据。测试用假 CLI 仅在隔离测试目录使用，不进入安装包。

最终安装包大小与 SHA-256 记录于 产物核验 和安装包旁的 `SHA256SUMS`。解包后的 Runner 与 Release 构建逐字节一致；主程序仅有 Tauri 正常写入的三字节 NSIS bundle type 标记差异（UNK → NSS），还原标记后逐字节一致。

额度显示调整后已重新构建安装包，最新大小与哈希以产物核验文件为准。ACL 修复后 Release 常规启动成功，窗口标题为 codebeat，持续存活且响应正常，启动 stderr 为空。

额度卡片将 300 分钟显示为「五小时窗口」、10080 分钟显示为「一周窗口」。百分比和进度条统一表示剩余额度（100% − 官方已用百分比），未知数据保持未知；底层保存的官方原始数据不变。

最终常规验证：**27 项 Rust 测试、8 项前端测试、2 项浏览器交互测试通过**；格式检查和 Clippy `-D warnings` 通过；另行显式运行的 Windows COM 测试与本机官方 CLI 只读探测均通过。两项机器环境测试在普通 `cargo test --workspace` 中标为 ignored，不能把它们混入常规测试计数。

### 可核对证据

- Rust 单元 / 集成测试：领域授权 / 时区、额度解析、IPC 协议、SQLite 多进程、独立 Runner 四类结果、进程树生命周期等。
- Clippy、格式检查、总验证日志。
- 前端测试、生产构建、浏览器交互测试、界面截图。
- Windows COM 集成测试、本机官方 CLI 只读探测、常规桌面启动进程状态。
- Runner Release、桌面 / NSIS、打包执行、安装包解包检查、npm audit。

日志保留实际输出，并统一为 UTF-8、将用户主目录遮挡为 `%USERPROFILE%`；其中 Windows PowerShell 的 `NativeCommandError` 包装可能出现在正常 Rust / Node stderr 消息前，判断依据是最终退出码、测试计数与 Finished 行，不把这些包装本身当作编译失败。汇总见 验证摘要。

### 本次修正的环境 / 编译问题

1. Rust 已安装但不在 PATH，构建脚本补入用户 cargo/bin。
2. MSVC 缺失，标准安装返回 1602；未提升权限，改用工作区 `.build-tools` 中从微软清单下载并校验哈希的便携工具。工具和缓存不进入源码与安装包。
3. Tauri 旧次版本与新 WebView2 依赖类型不兼容，统一至 Rust Tauri 2.12、JS API / CLI 2.12 并锁定依赖。
4. 修正托盘闭包中数据库变量遮蔽函数的编译错误与 Clippy 未使用 unit 表达式。
5. powercfg 权限不足原先中止全部诊断，改为保留可读结果及权限说明；COM 注册 / 查询 / 删除重测通过。
6. Vite 扫描 Rust target 中运行中的 EXE 导致 EBUSY / 请求卡住，已排除 target、工具、Rust 和文档目录，常规开发 HTTP 请求恢复 200。
7. npm 初始依赖存在已知漏洞，升级 Vite / Vitest 后审计为 0。
8. 补强执行日志帧上限、UNC 目标校验、JSON 密钥脱敏、失联结果不可被迟到成功覆盖。
9. Release 启动发现通用 Set-Acl 请求 SeSecurityPrivilege；改为从现有目录读取 Access 部分并仅写 DACL，不请求 SACL / Owner 权限。新增非管理员目录 ACL 测试，重复应用及文件读写均通过，见 ACL 测试。

自动审批拒绝过带远程调试参数的桌面启动，只提供 blocked by policy，未给具体原因；未继续该操作。改用普通启动、进程响应检查和独立浏览器测试验证，因此不声称做过桌面 WebView 的远程调试验收。

### 后续正式发布必须完成

执行 [Windows 人工验收矩阵](#windows-人工验收矩阵)，尤其真实账号续聊、Toast 展示、睡眠 / 重启、无 GUI 定时执行、干净 Win10/11 安装与卸载。还需实测预启动真实账号完整周期、完整国际化与首启向导、备份 / 迁移回滚体验、实时事件和完整许可证清单。未验收项没有勾选为完成。

## 品牌改名记录

显示名称为 **CodeBeat**；npm 包、Rust 桌面包、主程序和安装包使用 `codebeat`，独立运行器为 `codebeat-runner.exe`。工作目录为 `D:\open-source\codebeat`，远程仓库为 https://github.com/Shy2593666979/codebeat.git 。本次只初始化仓库并绑定 origin，不暂存、不提交、不推送。

### 统一内部名称

- Tauri 标识统一为 `org.codebeat.app`。
- 默认数据库为 `%LOCALAPPDATA%\codebeat\codebeat.db`，不自动读取或迁移旧品牌的数据目录。
- Task Scheduler 使用 `codebeat-` 前缀，开机启动注册表值名为 `codebeat`。
- 带应用标识的配置文件只接受 `codebeat`。
- 字体族为 `CodeBeat UI`，字体资源前缀为 `CodeBeatUI-`，Rust 包和测试标识同步统一。
- 项目尚未发布，不保留旧品牌兼容分支。历史构建日志和截图属于生成产物，重新验证时更新。

当前工作区占用原根目录、Git 元数据和历史日志目录，Windows 拒绝直接重命名。项目源文件和构建目录迁入新目录；旧路径通过目录连接/硬链接保留访问兼容，锁定的 Git 元数据和历史日志留作原路径副本。请后续打开 **新目录** 开发；新目录内有独立完整的 `.git` 与 origin 配置。两个仓库均无提交。旧路径用于兼容已有会话工作目录，不应再作为主开发目录。

`scripts/migrate-workspace-paths.ps1 -PreviousRoot <old> -CurrentRoot <new>` 只更新固定旧工作区路径且参数匹配的当前用户任务，注册前备份 XML，并验证触发器和设置不变；已有开机启动记录同样只更新路径。不会执行任务或调用模型。

## Windows 人工验收矩阵

此清单未执行项不得写为通过。虚拟机 CI 不能证明硬件唤醒可靠。

| 场景 | 验证方式 | 状态 |
|---|---|---|
| Win11 / Win10 x64 干净机器 | 不安装 SDK，安装 NSIS 后启动 UI、确认 runner 在同目录 | 待验收 |
| WebView2 缺失 | 验证下载安装引导、离线失败时提示 | 待验收 |
| 登录状态 | 两 CLI 缺失、仅一方、均安装、登录失效 | 待验收 |
| 实际任务完整链路 | 选择已知测试仓库会话，安排 5 分钟后执行，核对服务端结果、run 状态、日志与通知 | 待验收 |
| GUI 退出 | 任务注册后退出 GUI，确认 Runner 独立执行且不重复请求 | 待验收 |
| S3 / S0 / 休眠 | 实体机 AC / 电池，锁屏，检查 powercfg 与计划任务结果 | 待验收 |
| 错过策略 | 关机后重新登录，在 TTL 内 / 外分别测试三种策略 | 待验收 |
| 失联 | 模型已接受后杀 Runner；3 分钟后维护任务应标 needs_review，无自动重发 | 待验收 |
| 权限审批 | 工具请求未授权操作，确认不会自动扩大权限 | 待验收 |
| 中文路径 / OneDrive / 移动目录 | 检查会话验证、失败提示、无任意目录执行 | 待验收 |
| 时区与 DPI | UTC+8、DST、100% / 150% / 200%，键盘导航、深色 | 待验收 |
| 通知 | 确认当前用户 AUMID 通知、禁用通知 fallback、去重 | 待验收 |
| 卸载 / 升级 | 删除本应用计划任务和自启，保留数据库，CLI / 项目不受影响 | 待验收 |

正式发布前还需完整第三方许可证清单、签名方案、安全审查及真实账号兼容性记录。

## README 说明图生成提示词

生成方式：内置 `image_gen`，初次生成后针对时间线对齐进行一次局部调整。

图片位置：`docs/screenshots/reference/awo-explainer.png`。这是功能示意图，不是实际界面截图或实测结果。

### 初次生成提示词

Use case: infographic-diagram.
Asset type: a finished high-resolution Chinese explanatory infographic embedded in the GitHub README of CodeBeat, a Windows AI coding quota scheduling app. Generate a polished raster information poster, landscape roughly 3:2, preferably 2048 x 1360, sharp typography, no photographic background, no device mockup, no screenshot frame.

Design: elegant white and very pale ice-blue background, generous whitespace, deep navy #111032 text, electric blue #087BFF accents matching CodeBeat, delicate thin borders, soft rounded cards, tiny restrained mint and amber accents. Professional editorial SaaS diagram, clean Chinese sans-serif, large readable text, clear hierarchy. CodeBeat wordmark at top left with Code navy and Beat blue; small generic code-monitor line illustration optional, do not invent vendor logos. All text must be precise simplified Chinese, no gibberish. The image itself is the deliverable, with finished typography and complete diagrams.

Title exactly: "CodeBeat · 智能五小时额度优化"
Subtitle exactly: "按你的作息规划，随真实额度调整"
Small context badge: "示例：09:00–18:00，未启用午休"

Main composition: three numbered explanation cards stacked or arranged in a spacious 2-column editorial grid with a larger timeline card. Reading flow must be obvious.
Card 01 title: "设置你的工作时间"
Show tasteful clock icons beside "上班 09:00" and "下班 18:00".
Below: "午休可选 · 默认关闭". Tiny toggle visibly OFF.
Below: "支持跨午夜与当天临时安排".

Card 02 title: "搜索更合适的窗口"
Short exact caption: "比较有效工作时间、均衡程度与额外消耗".
Draw a precise horizontal timeline from 06:00 to 21:00. Working time only 09:00–18:00 as a separate mint band. Three equal five-hour predicted windows aligned to the same time axis: 06:00–11:00, 11:00–16:00, 16:00–21:00, outlined/dashed blue (predicted, not verified). Label main marks 06:00, 09:00, 11:00, 16:00, 18:00, 21:00, proportionate positions. Labels: "推荐首次触发 06:00", "有效工作 2 小时", "有效工作 5 小时", "有效工作 2 小时". Working overlaps are exactly 09–11, 11–16, 16–18. Avoid implying guaranteed new quota.

Card 03 title: "提前使用，计划跟着变"
Exact sentence: "若官方数据确认 05:00 已启动窗口".
A green solid verified segment "05:00–10:00" labelled "已确认窗口".
A clear cancellation chip with crossed-out 06:00, text "取消原定 06:00 预热".
Then an arrow leading to dashed blue recommendation segments "10:00–15:00" and "15:00–20:00" labelled "后续理论建议".
Small explanatory caption: "按真实重置时间重新规划，避免重复请求".
Clearly distinguish solid mint verified data and dashed blue theoretical predictions.

A compact separate note with amber lunch/clock icon:
Heading "午休不暂停五小时计时"
Text "11:00 启动 → 16:00 理论结束".
A tiny inset timeline 11 to16 with lunch12:00–13:30 hatched amber INSIDE a continuing 5-hour bracket, no pause symbol and no extended end time.

Bottom process ribbon, four steps with small icons/arrows, exactly:
"设置作息" → "搜索与评分" → "读取官方额度" → "必要时才触发"
Three restrained safeguards below: "已有窗口不重复触发" / "额度未知不自动执行" / "节省模式仅推荐".
Footer text readable, exactly: "理论边界不代表自动获得新额度，实际以官方数据为准。"

Constraints: This is a truthful conceptual explanation, not a fake screenshot or measured performance result. Never claim official guaranteed resets every five hours, never show reset-card redemption, never show percentage savings claims. Keep timeline math exact. No dense paragraphs, no watermarks, no extraneous English slogans. Use ample margins and consistent crisp line icons.

### 调整提示词

Edit only the geometry inside card 02 of this finished Chinese CodeBeat infographic. Preserve ALL other text, typography, cards, colors, layout, icons and exact canvas unchanged. Critical correction: use one shared linear time axis from 06:00 to 21:00. Keep the three predicted 5-hour bars equally wide and align their boundaries exactly to 06:00, 11:00, 16:00, 21:00. Position axis 09:00 at 20% of axis length and 18:00 at 80%. The mint green working-time line/band MUST begin exactly vertically above the 09:00 tick and end exactly above the 18:00 tick, not start later than 09:00. Correct the three mint effective-work brackets underneath to align exactly with 09:00–11:00 (2 hours), 11:00–16:00 (5 hours), 16:00–18:00 (2 hours); their lengths must have the ratio 2:5:2. Preserve their existing readable text labels. The current illustration's working band starts too far right and outer effective brackets are shifted. Fix that alignment precisely. No other changes anywhere. Preserve crisp readable simplified Chinese.
