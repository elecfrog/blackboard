+++
id = "000040"
lane = "bbq"
title = "Task Graph：引入类型化 In/Out Pin，替代四向自由连接"
created_at = "2026-05-09"
updated_at = "2026-05-10"
status = "archived"
area = "TaskGraph"
assignee = "codex"
depends_on = "000039"
kind = "feature"
parent = "000028"
+++

# 背景

当前 Task Graph 的连接模型是"四向自由 Pin"：
- `TaskGraphEdge` 只有 `from`/`to` + 可选 `source_handle`/`target_handle` 字符串
- 节点没有显式声明自己拥有哪些 Pin
- Pin 的方向（In/Out）、类别（执行流/数据流）、数据类型全部隐式
- 前端渲染时连线无法区分控制流和数据流，复杂图一眼看不懂

参考 UE 蓝图的设计：
- 每个节点**显式声明** InPin 和 OutPin
- Pin 分两大类：**Exec**（执行流，白色三角 ▶）和 **Data**（数据流，彩色圆点 ●）
- Data Pin 带有明确的 `value_type`（string / int / bool / json / array 等）和 `var_name`
- 连线只能从 OutPin → InPin，类型必须兼容
- 视觉上一目了然：哪些是控制流走向，哪些是数据传递

# 目标

1. **后端**：在 Node 模型中引入显式 Pin 声明，Edge 模型绑定到具体 Pin ID
2. **前端**：节点渲染 InPin（左侧）和 OutPin（右侧），按类型着色，连线有方向约束
3. **兼容**：保持对现有 system graph JSON 的向后兼容（老格式可自动升级）

# 设计方案

## 1. Pin 数据模型（后端 types.rs）

```rust
/// Pin 的类别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinCategory {
    /// 执行流 Pin（控制流走向）
    Exec,
    /// 数据流 Pin（传递变量值）
    Data,
}

/// Pin 的方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    In,
    Out,
}

/// 数据 Pin 的值类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinValueType {
    String,
    Int,
    Float,
    Bool,
    Json,
    Array,
    Any,
}

/// 节点上的一个 Pin 声明
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePin {
    /// Pin 唯一 ID（节点内唯一，如 "exec_in", "loop_body", "index"）
    pub id: String,
    /// 显示标签
    pub label: String,
    /// 方向
    pub direction: PinDirection,
    /// 类别
    pub category: PinCategory,
    /// 数据类型（仅 category=Data 时有意义）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value_type: Option<PinValueType>,
    /// 是否必须连接
    #[serde(default)]
    pub required: bool,
}
```

## 2. Node 模型变更

```rust
pub struct TaskGraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub label: String,
    pub description: Option<String>,
    pub position: Option<Position>,
    pub config: serde_json::Value,
    /// 显式 Pin 声明（新增）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pins: Vec<NodePin>,
}
```

## 3. Edge 模型变更

```rust
pub struct TaskGraphEdge {
    pub id: String,
    pub from: String,          // 源节点 ID
    pub to: String,            // 目标节点 ID
    pub from_pin: String,      // 源节点的 OutPin ID
    pub to_pin: String,        // 目标节点的 InPin ID
    /// 保留 kind 用于快速过滤
    pub kind: EdgeKind,
    pub label: Option<String>,
    // 废弃 source_handle / target_handle，由 from_pin / to_pin 替代
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// 执行流连线
    Exec,
    /// 数据流连线
    Data,
}
```

## 4. 各节点类型的默认 Pin 定义

| NodeType | InPins | OutPins |
|----------|--------|---------|
| **Start** | — | `exec_out` (Exec) |
| **End** | `exec_in` (Exec) | — |
| **Llm** | `exec_in` (Exec) | `exec_out` (Exec), `output` (Data/Json) |
| **RegisteredTask** | `exec_in` (Exec) | `exec_out` (Exec), `output` (Data/Json) |
| **Branch** | `exec_in` (Exec) | 每条 rule 一个 `rule:{id}` (Exec) |
| **Loop** | `exec_in` (Exec) | `body` (Exec), `exit` (Exec), `index` (Data/Int) |
| **HumanGate** | `exec_in` (Exec) | `exec_out` (Exec), `action` (Data/String) |

## 5. 前端渲染规则

- InPin 渲染在节点**左侧**，OutPin 渲染在节点**右侧**
- Exec Pin：白色/灰色三角图标 ▶，连线为实线
- Data Pin：彩色圆点 ●，颜色按 value_type 区分（string=粉色, int=青色, bool=红色, json=橙色）
- Pin 旁显示 label 文字
- 拖拽连线时只允许 OutPin → InPin，且 category 必须匹配（Exec→Exec, Data→Data）
- Data 连线额外校验 value_type 兼容性（Any 可接受任意类型）

## 6. 向后兼容策略

- 读取旧格式 JSON 时：如果 node 没有 `pins` 字段，根据 `node_type` 自动生成默认 pins
- 如果 edge 有 `source_handle`/`target_handle` 但没有 `from_pin`/`to_pin`，自动映射：
  - `source_handle: "body"` → `from_pin: "body"`
  - `source_handle: "exit"` → `from_pin: "exit"`
  - `source_handle: "rule:xxx"` → `from_pin: "rule:xxx"`
  - 无 handle → `from_pin: "exec_out"`, `to_pin: "exec_in"`
- 保存时统一写新格式

# 交付拆分

1. **后端 Pin 模型** — types.rs 新增 Pin 相关类型，Node/Edge 结构变更
2. **后端兼容层** — 读取旧 JSON 时自动升级，validation 校验 Pin 连接合法性
3. **后端 interpreter 适配** — 用 pin ID 替代 source_handle 做路由
4. **前端 Pin 渲染** — 节点组件渲染 In/Out Pin，区分 Exec/Data 样式
5. **前端连线约束** — 拖拽连线时的类型校验与视觉反馈
6. **前端数据流可视化** — Data 连线显示变量名和类型标注

# 非目标（本轮不做）

- 不做 Pin 的动态增减（如 UE 的 "Add pin +"）
- 不做数据流的运行时求值（数据 Pin 暂时只做可视化，实际数据传递仍走 context）
- 不做类型转换节点

# 当前进展

- 2026-05-09：默认 Loop pins 从四向自由连接改为只暴露 exec_in、body、exit、index

# 记录

- 来源：inbox/2026-05-09-codex-task-graph-loop-frame.md
