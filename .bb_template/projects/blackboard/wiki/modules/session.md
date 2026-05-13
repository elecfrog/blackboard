# Session 模块

单文件模块示例，用来验证：

- 顶层 `modules/session.md` 的路径解析
- 从嵌套页面用 `../session.md` 回跳（见 [context-management/index.md](./context-management/index.md)）
- 代码块与任务清单渲染

## 任务清单

- [x] 文件上传
- [x] 只读浏览
- [ ] 版本化 / 历史对比（延后单）

## 代码块

```rust
pub fn read_wiki_content(&self, file_path: &str) -> Result<WikiContentResponse, InboxError> {
    let safe_path = self.validate_wiki_path(file_path, ValidateKind::Document)?;
    // ...
}
```

回到 [入口](../index.md)。
