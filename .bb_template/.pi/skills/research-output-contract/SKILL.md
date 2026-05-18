---
name: research-output-contract
description: Enforce TaskGraph research output placement and writer ownership.
---

# Research Output Contract

When a TaskGraph prompt includes a `resource_bundle`, treat its contracts as
binding.

- Scouts and synthesizers may investigate and return structured output, but
  must not write the final research report.
- The final research report must be written by the graph's system writer node.
- The final output path must be under
  `resource_bundle.contracts.final_output_dir`; in the current workspace this
  directory is normally `wiki/research`.
- If `resource_bundle.contracts.final_output_path` is present, use it as the
  intended final path and keep it inside `wiki/research`.
- Temporary exploration files, if needed, must stay under
  `resource_bundle.runtime.scratch_dir`.
