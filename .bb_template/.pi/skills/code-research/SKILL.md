---
name: code-research
description: Use structured resource bundles for code research TaskGraph nodes.
---

# Code Research Skill

When the prompt includes a `resource_bundle`, treat it as the authority for task
inputs and data locations.

- Use `resource_bundle.task.input` as the user request.
- Use `resource_bundle.data_sources.source_root` as the root for code or
  document lookup.
- Use `resource_bundle.data_sources.wiki_slice` only as supplemental context.
- Treat candidate paths as relative to `source_root`, not to the agent cwd.
- Do not write or modify final research files unless the node explicitly says
  it is the system writer.
- Final research reports must be written under
  `resource_bundle.contracts.final_output_dir` when present; for Blackboard
  TaskGraph research this is `wiki/research`.
- Return evidence with stable source anchors whenever possible.
