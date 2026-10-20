# README 宣传截图

此目录包含当前 Vue 界面的浏览器截图，以及一张 AI 绘制的额度排程对比图，供仓库 README 展示，随源码保留。应用界面截图未使用 AI 生成。

任务、会话、额度、重置卡与统计均为演示数据；项目目录统一使用 `C:\Projects\demo-app`。截图不包含真实会话、账号或本机工作目录。Claude Code 的额度保持未知，避免暗示支持尚不可用的读取能力。

| 文件 | 内容 |
|---|---|
| `home.png` | 首页额度概览、今日状态与 Token 趋势 |
| `tasks.png` | 浅色任务管理，一次性与周期任务、暂停与待确认状态 |
| `new-task.png` | 新建任务、会话选择、提示词和锁定策略 |
| `coding-allocation.png` | 每日额度调配与共享时间尺 |
| `usage.png` | 额度监控、恢复时间与 Token 趋势 |
| `tasks-dark.png` | 深色任务管理 |
| `coding-quota-comparison-ai.png` | AI 排程示意：10:00–20:00 工作时段中，普通使用与提前安排窗口的对比 |

AI 对比图使用内置 image_gen 工具生成，经过一次时间轴布局校正。它描述一种示例安排，不承诺额度增加或固定收益。原始提示词与校正提示词保存在 [coding-quota-comparison-prompt.md](coding-quota-comparison-prompt.md)。下方脚本仅重新生成应用截图，不会覆盖此 AI 图片。

## 重新生成

在仓库根目录运行：

```powershell
npm --prefix desktop ci
npm --prefix desktop run build
Push-Location desktop
npx playwright test tests/ui/readme-gallery.spec.ts --workers=1
Pop-Location
```

首次使用 Playwright 时，按其提示安装 Chromium。截图测试在浏览器加载前注入桌面调用夹具，不启动真实桌面服务，不注册 Windows 计划任务，也不发送模型请求。修改界面后请重新生成并检查截图。
