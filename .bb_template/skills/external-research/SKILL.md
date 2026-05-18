---
name: external-research
description: Use structured resource bundles for generic external search/research TaskGraph nodes.
---

# External Research Skill

When the prompt includes a `resource_bundle`, treat it as the authority for
research intent, source priority, and output ownership.

- Use `resource_bundle.task.input` as the research question.
- Prioritize `resource_bundle.data_sources.source_urls` before following
  secondary links, market data pages, news links, or search results.
- Valid sources include repositories, official docs, external articles, papers,
  wiki pages, issues, releases, news, datasets, and market/finance pages.
- Prefer primary or official sources, repository files, README/docs, examples,
  releases, source code, filings, datasets, or directly cited articles over
  summaries.
- For time-sensitive or market/news claims, preserve the publication time,
  data timestamp, ticker/market context, and source date when available.
- Clearly separate source facts from inference.
- Include stable `source_anchors` for key claims: URLs, repository file paths,
  headings, tags, commits, docs, papers, wiki pages, issues, news articles,
  datasets, or finance source anchors.
- Do not write the final report; return structured scout output or final
  Markdown to the graph runtime.
- If temporary files are needed for exploration, keep them under
  `resource_bundle.runtime.scratch_dir`.
