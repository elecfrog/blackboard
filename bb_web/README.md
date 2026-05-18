# Blackboard Web

本地 Blackboard 看板，强依赖 `bb_server` HTTP REST API facade。前端不维护离线降级路径——`bb_server` 不可达时显示错误提示。

## Architecture: Index / Content Split

前端采用索引/内容分离架构：

- **列表（看板）**：通过 `GET /api/projects/{project}/tickets` 获取 ticket 索引（id、lane、title、status、timestamps、dependencies、extra），不含正文 content。
- **详情（按需加载）**：当用户打开 ticket 详情时，通过 `GET /api/projects/{project}/tickets/{id}/content` 按需获取 Markdown 正文。

JSON 索引持久化在 workspace registry 解析后的 project 数据目录中，由权威 `bb_daemon` 在运行时动态维护（启动时自动构建，每次 CRUD 操作后同步更新）。Markdown 做内容，JSON 做索引，两者在同一目录结构内共存。

## Usage

```bash
cd ..
python3 scripts/setup.py
python3 scripts/dev.py
```

`vite dev` 会把 `/api/*` 反代到 `http://127.0.0.1:3002` 的 `bb_server`，再由 `bb_server` 代理到 `http://127.0.0.1:3001` 的 `bb_daemon`。
`scripts/dev.py` 默认以 repo 根目录 `.bb_template` 启动 `bb_daemon`、`bb_server`、Vite 三个进程；需要模拟 Desktop 用户级 `~/.bb` 时使用 `python3 scripts/dev.py --user-root` 或 `BB_DEV_USER_ROOT=1 python3 scripts/dev.py`。

```bash
npm run dev
```

生产/desktop 冒烟可以让 `bb-daemon serve` 直接服务构建产物：

```bash
npm run build
cargo run --manifest-path ../bb_backend/Cargo.toml -p bb_daemon -- --root ~/.bb serve --addr 127.0.0.1:3001 --static-dir dist
```
