# Agents UI chunk warning cleanup

时间: 2026-05-16T14:52:30Z
来源: codex
项目: blackboard

## 做了什么

- 将 BoardView/HomeView 改为路由懒加载，并将 Agents/Inbox/Wiki/TaskGraph 等工作区面板改为 async component，降低首包耦合。
- 将 AgentChatFab 改为异步加载，并移除 @tdesign-vue-next/chat 运行时依赖，改用轻量本地 chat DOM/CSS。
- 重写 Vite manualChunks，把 Vue、TDesign、icons、markdown、highlight、Mermaid、Cytoscape 等 vendor 拆分；Mermaid core 懒加载 chunk 稳定在 520k 预算内。

## 验证了什么

- npm run build --prefix bb_web: 通过，未再输出 large chunk warning。
- Browser(iab) http://localhost:8060/#/projects/blackboard/agents: Agents profile 存在，chat fab 可见。
- Browser(iab) 打开 chat fab: 面板、textarea、发送按钮正常渲染，旧 TDesign chat DOM 为 0。

## 下一步

- 若后续继续压缩首屏，可再评估按 workspace 级别拆分 TDesign CSS 或对 Mermaid 渲染入口做更细粒度按需加载。

## 相关位置

- bb_web/src/main.ts
- bb_web/src/views/BoardView.vue
- bb_web/src/App.vue
- bb_web/src/components/AgentChatFab.vue
- bb_web/src/i18n.ts
- bb_web/src/styles.css
- bb_web/vite.config.ts
