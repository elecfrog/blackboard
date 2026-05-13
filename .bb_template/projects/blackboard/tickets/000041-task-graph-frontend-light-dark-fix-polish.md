+++
id = "000041"
lane = "bbd"
title = "Task Graph 前端：frontend-light-dark-fix 编辑页视觉优化与错误修复"
created_at = "2026-05-09"
updated_at = "2026-05-10"
status = "archived"
area = "TaskGraph"
assignee = "codex"
depends_on = "000039"
kind = "frontend-polish"
parent = "000028"
scope = "bb_web-task-graph-editor"
+++

# 当前进展

- 交付目标：修复 frontend-light-dark-fix Task Graph 页面明显视觉炸裂、布局错位和前端错误，恢复可读可编辑体验。

- 2026-05-09：收紧 Task Graph 详情路由布局，压缩编辑器/运行面板大面积空白
- 2026-05-09：移除 graph 卡片节点/边统计卡及预览页底部 meta
- 2026-05-09：将 graph 卡片运行入口改成大按钮，展示运行动作/状态/最近运行时间
- 2026-05-09：合并 RunPanel running/started/updated/elapsed/cursor 到标题区，running 图标增加动画
- 2026-05-09：将 Task Graph mutation 异常改为 Alert 弹窗，移除全局 message 干扰

- 2026-05-09：限制编辑态 catalog refresh 只刷新目录与运行历史，不再重载当前 graph
- 2026-05-09：进入 edit 路由时清空 activeRunId，避免运行日志 stream 抢走编辑面板

- 2026-05-09：TaskGraphCatalogPanel.vue 替换 hardcoded #fff 为 var(--bb-surface)，修复 dark mode 渲染
- 2026-05-09：MarkdownRenderer.vue 添加 [data-theme='dark'] dark mode 语法高亮覆盖

- 2026-05-09：TaskGraphRunPanel.vue 替换 hardcoded 中英文为 t() 调用
- 2026-05-09：TaskGraphEditorPanel.vue 替换 6 个 hardcoded 按钮/标签为 t() 调用
- 2026-05-09：新增 18 个 i18n key 到 bb_web/src/i18n.ts

- 2026-05-10：修复 end 节点可接收连入连接
- 2026-05-10：新增 pipeline i18n key 供 sub-pipeline inspector 使用

- Codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-input-layout-standardization.md`。

- Codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-binding-list-layout-standardization.md`。

- Codex 完成阶段工作，handoff 写入 `2026-05-10-codex-bb-web-design-dense-editor-guidelines.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-inputs-component-split.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-binding-list-component-split.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-run-inputs-component-split.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-shared-ui-helper-splits.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-pins-domain-split.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-bb-web-design-recurring-component-pitfalls.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-node-delete-action.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-create-modal-entry.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-header-action-alignment-design-language.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-catalog-card-density-unification.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-preview-action-placement.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-catalog-run-status-square.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-catalog-title-density-polish.md`。

- codex 开始执行：继续排查 Task Graph 子流水线 input_bindings 中 max-iterations 为空的问题。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-sub-pipeline-input-binding-空值修复.md`。

- 2026-05-10: 统一 Task Graph catalog 卡片密度规则，收敛 detail 路由小卡片尺寸为默认规格，沉淀到 DESIGN.md

- 2026-05-10: 将 catalog-card 运行状态从 footer 移至卡片 header 36px 状态方块，idle/pending/running/paused/succeeded/failed/cancelled 对应不同图标，移除重复卡片可见运行文字

- 2026-05-10: 缩小 Task Graph catalog 卡片标题字号与字重，紧凑标题与状态 badge 间距

- 2026-05-10: 将选中 graph 的 View/Edit/Customize 操作移至 preview header，放在 Run 按钮左侧；从左侧 catalog 卡片移除重复操作控件

- 2026-05-10: 将节点配置面板删除入口从 icon-only 改为带文案按钮；增加 Delete/Backspace 快捷键删除选中节点，同步移除关联边并清空选中状态

- 2026-05-10: 移除顶部持久创建横条表单，将新建 Project Graph 按钮移至 header；在 catalog 新增 modal card 创建流程，支持自动聚焦、Escape/背景关闭、提交后跳转编辑页

- 2026-05-10: 移除冗余的本地刷新入口，统一用全局 topbar 刷新；修复 top action 按钮盒模型使 icon-label 居中稳定；修复 UnifiedPopupSelect change handler 阻塞构建的 typo

- 2026-05-10: 新增 taskGraphPins.ts 承接 pin 颜色、默认生成、连接判定、canvas 投影和节点高度计算；taskGraphs.ts 保留 facade 导出

- 2026-05-10: 抽出 TaskGraphBindingList.vue 统一 prompt_vars 和 input_bindings 的 key/value 行、删除按钮和快捷绑定按钮；TaskGraphEditorPanel.vue 改为复用该组件

- 2026-05-10: 抽出 TaskGraphRunInputsPanel.vue 把运行输入预览控件、引用展示和盒模型样式集中到独立组件；TaskGraphCatalogPanel.vue 改为传入 inputs 与当前输入值

- 2026-05-10: 把流水线 Inputs 改为每个变量一整行，统一 cell 结构与盒模型避免溢出；Inputs 和 Graph 设置从并排改为上下两行布局

- 2026-05-10: 把 registered_task 节点的 prompt_vars 改为标准 binding row；key/value/删除按钮固定主行，引用按钮改为下一行 chip；sub_pipeline input_bindings 复用同一样式

- 2026-05-10: 抽出 TaskGraphInputsPanel.vue 把流水线 Inputs 模板、控件标准化逻辑和局部样式从 editor 主文件移出；EditorPanel 改为传入 graphInputs / isReadonly 并监听事件

- 2026-05-10: MarkdownRenderer.vue 复用 slugifyHeading 消除 heading id 生成重复；新增 ui/wiki/render.ts 把 Wiki 渲染类型和 code markdown 包装逻辑从 WikiPanel.vue 移出；新增 views/boardViewUtils.ts 把 BoardView 状态转换和依赖解析移出

- 2026-05-10: 在 DESIGN.md 新增 Component Ownership And Reuse 规范和 Recurring Small Pitfalls 总结，补充 Graph Workbench 和 Implementation Checklist

- 2026-05-10: 修正 bbpm-maintain 和 bbpm-maintain-custom 中 max-iterations 空绑定为 {{inputs.inbox-max-iterations}}；让 sub_pipeline input_bindings 支持父 graph inputs 快捷绑定；在前端保存校验拦截空字符串 sub-pipeline input binding

- 2026-05-10: 在 DESIGN.md 补充 Notion-like productivity workbench 产品姿态说明；新增 Box Model And Control Grammar、Dense Editor Rows、Graph Workbench 规范；扩展 Implementation Checklist

- 2026-05-10: 修复 Task Graph 列表 failed 运行按钮样式为浅红底；将 route detail 左侧 catalog 改为局部滚动，按右侧 preview 高度收敛滚动条；补齐 i18n 类型索引

- codex 开始执行：继续优化 Task Graph Run header：移除 Run UI/API 噪声与重复状态，并将 cursor 显示为当前节点。

- 2026-05-10: 新增 --bb-theme-* 黑白主题语义 token 并桥接 TDesign 品牌色变量；将 Inbox 侧边栏 Badge、Task Graph 筛选 active、graph 选中态、run input reference 从青/蓝色收敛到黑白主题语义

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-run-header-信息降噪.md`。

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-run-header-按钮盒模型修复.md`。

- 2026-05-10：移除 Run header 中 Run UI/API 标题、重复状态 pill，将 cursor UUID 替换为当前节点 label · type

- codex 完成阶段工作，handoff 写入 `2026-05-10-codex-task-graph-run-header-rounded-box-规范.md`。

- 2026-05-10：收敛 Task Graph Run header 操作按钮与元信息 chip 圆角为 6px rounded box，统一 danger outline 按钮样式

- 2026-05-10：移除顶部工具栏皮肤下拉入口，保留 Blackboard skin preset 作为内部主题基础设施；新增 task graph reference chip 语义 token；将编辑态 task-graph-variable-chip 与预览态 run input reference 统一到同一组黑白主题 token

# 记录

- 来源：用户指出 http://127.0.0.1:8060/#/projects/blackboard/task-graphs/system/frontend-light-dark-fix 页面明显炸裂，要求在 039 后追加前端优化单并用 Computer Use 验收。

- 来源：inbox/2026-05-09-codex-taskgraph-frontend-polish.md

- 来源：inbox/2026-05-09-codex-task-graph-edit-refresh-guard.md

- 来源：inbox/2026-05-09-opencode-dark-mode-agent-dark-mode-fixes-for-bb-web-components.md

- 来源：inbox/2026-05-09-opencode-i18n-scan-fix-agent-前端-i18n-检查与修复-第一批.md

- 来源：inbox/2026-05-10-codex-taskgraph-end-edge.md

- 验证：npm run build --prefix bb_web 通过。
- 验证：Computer Use 在 Chrome 的 project/inbox-batch-cleanup-custom/edit 页面确认 Inputs 与 Graph 设置已上下分行，input 行未溢出。

- 验证：npm run build --prefix bb_web 通过。
- 验证：git diff --check -- bb_web/src/components/TaskGraphEditorPanel.vue bb_web/src/i18n.ts 通过。
- 验证：Computer Use 在 Chrome 的 project/inbox-batch-cleanup-custom/edit 页面确认 prompt_vars 不再横向挤压。

- 验证：git diff --check -- bb_web/DESIGN.md 通过。

- 验证：`npm run build --prefix bb_web` 通过。

- 验证：`npm run build --prefix bb_web` 通过。

- 验证：`npm run build --prefix bb_web` 通过。

- 验证：`npm run build --prefix bb_web` 通过。

- 验证：`npm run build --prefix bb_web` 通过。

- 验证：`git diff --check -- bb_web/DESIGN.md` 通过。
- 验证：本次仅修改设计文档，未触及 `bb_web/src`，未运行前端 build。

- 验证：`npm run build --prefix bb_web` 通过。
- 验证：`git diff --check -- bb_web/src/components/TaskGraphEditorPanel.vue bb_web/src/i18n.ts` 通过。

- 验证：`npm run build --prefix bb_web` 通过。
- 验证：`git diff --check -- bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/src/i18n.ts bb_web/DESIGN.md` 通过。
- 验证：当前会话未获得可用的 in-app browser 自动化接口，因此未做浏览器截图验证。

- 验证：PASS: git diff --check -- bb_web/src/styles.css bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/src/components/UnifiedPopupSelect.vue bb_web/DESIGN.md
- 验证：PASS: npm run build --prefix bb_web

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/DESIGN.md bb_web/src/styles.css bb_web/src/components/UnifiedPopupSelect.vue
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: Browser route load check for /#/projects/blackboard/task-graphs/ and /#/projects/blackboard/task-graphs/system/bbpm-maintain

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/DESIGN.md bb_web/src/styles.css bb_web/src/components/UnifiedPopupSelect.vue
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: Browser route check confirmed preview header exposes 查看 / 编辑 / Customize / 运行

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/DESIGN.md bb_web/src/styles.css bb_web/src/components/UnifiedPopupSelect.vue
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: Browser route check confirmed repeated catalog run controls expose aria labels and preview header keeps 查看 / 编辑 / Customize / 运行

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphCatalogPanel.vue bb_web/DESIGN.md bb_web/src/styles.css bb_web/src/components/UnifiedPopupSelect.vue
- 验证：PASS: npm run build --prefix bb_web

- 验证：PASS: jq empty task_graphs/system/bbpm-maintain.json
- 验证：PASS: jq empty projects/blackboard/task_graphs/bbpm-maintain-custom.json
- 验证：PASS: rg confirmed no remaining "max-iterations": "" in touched graph files
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: curl http://127.0.0.1:8060/api/projects/blackboard/task-graphs/system/bbpm-maintain returned max-iterations={{inputs.inbox-max-iterations}}

- 来源: inbox/2026-05-10-codex-task-graph-catalog-card-density-unification.md

- 来源: inbox/2026-05-10-codex-task-graph-catalog-run-status-square.md

- 来源: inbox/2026-05-10-codex-task-graph-catalog-title-density-polish.md

- 来源: inbox/2026-05-10-codex-task-graph-preview-action-placement.md

- 来源: inbox/2026-05-10-codex-task-graph-node-delete-action.md

- 来源: inbox/2026-05-10-codex-task-graph-create-modal-entry.md

- 来源: inbox/2026-05-10-codex-task-graph-header-action-alignment-design-language.md

- 来源: inbox/2026-05-10-codex-task-graph-pins-domain-split.md

- 来源: inbox/2026-05-10-codex-task-graph-binding-list-component-split.md

- 来源: inbox/2026-05-10-codex-task-graph-run-inputs-component-split.md

- 来源: inbox/2026-05-10-codex-task-graph-input-layout-standardization.md

- 来源: inbox/2026-05-10-codex-task-graph-binding-list-layout-standardization.md

- 来源: inbox/2026-05-10-codex-task-graph-inputs-component-split.md

- 来源: inbox/2026-05-10-codex-shared-ui-helper-splits.md

- 来源: inbox/2026-05-10-codex-bb-web-design-recurring-component-pitfalls.md

- 来源: inbox/2026-05-10-codex-task-graph-sub-pipeline-input-binding-空值修复.md

- 来源: inbox/2026-05-10-codex-bb-web-design-dense-editor-guidelines.md

- 来源: inbox/2026-05-10-codex-task-graph-catalog-scroll-failed-style.md

- 来源: inbox/2026-05-10-codex-blackboard-黑白主题色系统与-task-graph-主题色收敛.md

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphRunPanel.vue bb_web/src/i18n.ts
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: Browser DOM check confirmed no Run UI/Cursor text and header shows 当前节点 Batch Inbox Cleanup · llm

- 验证：PASS: git diff --check -- bb_web/src/components/TaskGraphRunPanel.vue bb_web/src/i18n.ts
- 验证：PASS: npm run build --prefix bb_web
- 验证：PASS: Browser DOM check confirmed Run header still exposes 当前节点 and action buttons without Run UI/Cursor noise

- 来源: inbox/2026-05-10-codex-task-graph-run-header-信息降噪.md (已清理)

- 验证：npm run build --prefix bb_web 通过。
- 验证：git diff --check -- bb_web/src/components/TaskGraphRunPanel.vue bb_web/DESIGN.md bb_web/src/i18n.ts 通过。
- 验证：浏览器已打开 http://localhost:8060/#/projects/blackboard/task-graphs/system/inbox-batch-cleanup；当前页面没有 active run header，因此本轮没有强行启动流水线做视觉状态验证。

- 来源：inbox/2026-05-10-codex-task-graph-run-header-rounded-box-规范.md（已清理）

- 来源：inbox/2026-05-10-codex-task-graph-黑白主题收敛.md
- 相关路径：bb_web/src/views/BoardView.vue, bb_web/src/styles.css, bb_web/src/components/task-graph/TaskGraphInputsPanel.vue, bb_web/src/components/task-graph/TaskGraphRunInputsPanel.vue, bb_web/TDESIGN.md

# 下一步

- 用 Computer Use 对页面做视觉检查，归因并修复 Task Graph 前端布局/样式/交互问题。
