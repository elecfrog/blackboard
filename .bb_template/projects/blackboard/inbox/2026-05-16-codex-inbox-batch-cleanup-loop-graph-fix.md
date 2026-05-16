# Fixed inbox-batch-cleanup stale loop body references

时间: 2026-05-16T13:34:51Z
来源: codex
项目: blackboard

## 做了什么

- Updated `.bb_template/task_graphs/system/inbox-batch-cleanup.json` so loop `body_entry`, `body_exit`, and `condition.input_ref` point to the actual LLM body node `b7ffec9f-2a60-4e25-9832-baba8485fecc` instead of stale node id `3d0b8ead-4ec2-43be-a087-b1221159694a`.
- Confirmed the explicit LLM-to-loop edge is not needed because the compiler already attaches a hidden loop-return writer from `body_exit`; adding such an edge would create a multi-incoming barrier on the loop node.

## 验证了什么

- Parsed the JSON with PowerShell `ConvertFrom-Json`: passed.
- Ran `bb.exe --root D:\Dev\blackboard\.bb_template daemon --once --dry-run --project blackboard --graph system/inbox-batch-cleanup`: passed; created run `run-20260516-133432-02e1fdc0` with status `succeeded`.
- Inspected compiled graph for dry-run and confirmed the LLM body process writes hidden channel `branch:to:3db02982-0728-4a77-8763-c73eec9658b7`, allowing loop re-entry.

## 下一步

- （未填写）

## 相关位置

- .bb_template/task_graphs/system/inbox-batch-cleanup.json
- .bb_template/runtime/task_graph_runs/blackboard/run-20260516-133432-02e1fdc0/run.json
- .bb_template/runtime/task_graph_runs/blackboard/run-20260516-133432-02e1fdc0/graph.compiled.json
