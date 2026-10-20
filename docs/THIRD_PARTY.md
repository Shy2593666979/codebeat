# 第三方说明

CodeBeat UI 字体由 Noto Sans SC 派生，保留完整字符覆盖并生成 400/500/600/700 四个静态字重，重命名为应用私有字体族。Copyright 2014-2021 Adobe，SIL Open Font License 1.1；完整许可随安装包放在 `fonts/OFL.txt`，源版本与文件哈希见 `fonts/source.json`。上游：https://github.com/google/fonts/tree/main/ofl/notosanssc 。字体独立适用 OFL，不受项目代码许可证替代。

本产品不是 OpenAI 或 Anthropic 官方产品，不附带 Codex 或 Claude Code 官方二进制。用户须自行安装、登录并遵守各工具的服务条款、订阅及区域限制。

核心依赖：Rust / Tokio（MIT）、Tauri（MIT / Apache-2.0）、Vue（MIT）、Vite（MIT）、rusqlite（MIT）、SQLite（Public Domain）、Lucide（ISC）。完整版本在 Cargo.lock / package-lock.json 中。正式分发前应生成完整许可证清单并核对包含的依赖。

界面中的 OpenAI 图形来自 Simple Icons 11.15.0 的 openai.svg（CC0-1.0）：https://github.com/simple-icons/simple-icons/blob/11.15.0/icons/openai.svg 。商标权仍归原权利人所有，仅用于识别工具提供方，不表示官方背书。

Claude 橙色星芒图形来自 Simple Icons 的 claude.svg（CC0-1.0，2026-10-16 获取）：https://github.com/simple-icons/simple-icons/blob/develop/icons/claude.svg 。本地文件 `desktop/src/assets/claude.svg` 保留路径与比例，以品牌色 `#d97757` 显示；用于 Claude Code 提供方识别，商标归 Anthropic 所有。OpenAI 图形改用原黑白配色；上述小型 SVG 随应用离线提供，不依赖网络图片。

desktop/assets/brand/logo.png 与界面参考图 由项目发起人提供，仅视作本项目实现授权；不声明对外部商标的权利。发布方需确认视觉素材的再分发权。
