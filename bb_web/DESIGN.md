# Blackboard Web Design

Blackboard is a Notion-like workbench for coordinating local AI coding agents. The interface should feel quiet, information-dense, and durable: closer to a working database view than a marketing page.

This file is the design source of truth for `bb_web`. Use it when adding or polishing UI.

TDesign integration rules live in [`TDESIGN.md`](./TDESIGN.md). When using or adding TDesign components, follow that file alongside this design source of truth.

## Product Posture

- Build the usable work surface first. Do not add landing-page heroes, illustration sections, or marketing-style feature cards.
- Optimize for scanning, comparison, repeated action, and low cognitive load.
- Prefer calm prose that explains what a surface helps the user decide. Put interaction instructions in toolbars, empty states, or tooltips.
- Do not expose machine-specific absolute paths in general product copy. Paths are acceptable only in explicit diagnostics, sync targets, or copy-path controls.
- Treat Blackboard as a Notion-like productivity workbench, not a generic admin panel. It should feel quiet and composed, but also fast for repeated tool use.
- Design language is more than color tokens. Layout rhythm, box sizing, row density, overflow behavior, and interaction grouping are part of the theme.
- Dense tools should feel intentional. If a component needs several fields, design the field grammar first instead of letting raw DOM order decide the layout.

## Visual System

### Color

Blackboard's brand surface is black and white. Use the teal accent only where it represents product semantics that already carry domain meaning, such as existing lane/status colors. Do not use teal or blue as the default selected state for navigation, agent selection, connector rows, or primary actions.

- Canvas: `#f6f5f2`
- Surface: `#ffffff`
- Surface soft: `#fafaf8`
- Surface muted: `#f1f0ed`
- Hairline: `#e7e4de`
- Hairline strong: `#cbc5bc`
- Text strong: `#191714`
- Text: `#37352f`
- Text muted: `#78736b`
- Text faint: `#a19b92`
- Accent: `#0f766e`
- Accent soft: `#e7f4f1`
- Blue focus: `#2563eb`
- Error: `#b91c1c`
- Warning: `#b45309`
- Success: `#059669`
- Blackboard mark light: black background with white foreground.
- Blackboard mark dark: white background with black foreground.

Use color sparingly. Let status colors and lane colors carry workflow meaning; let the Blackboard black/white pair carry brand emphasis and selected UI chrome.

### Dark Mode

- Dark mode must preserve the same information architecture and color logic as light mode. Do not solve dark contrast by dropping semantic color, changing which part of an object is colored, or making a surface feel like a different product.
- When a component uses a colored status treatment in light mode, dark mode should mirror the treatment with adjusted contrast. For graph nodes, the chosen pattern is whole-node status tint plus a stronger left status accent, not a light-only accent bar.
- Selected navigation, selected agent rows, connector selected rows, primary sync actions, and count badges use the black/white brand pair in both themes.
- Avoid dark-mode teal/blue residue in generic selection states. Teal/blue is allowed for domain status, focus rings, graph edges, and ticket/lane semantics.
- Pin handles and small interactive affordances need visible rings in dark mode even if those rings are subtle in light mode.

### Selection And Badges

- Generic selected states use neutral surfaces, hairline borders, and the Blackboard black/white accent. Examples: sidebar active item, agent list active item, agent hero, connector target row.
- Primary action buttons inside Settings and connector management use the Blackboard black/white pair, not blue.
- Count badges should only appear where the number changes the user's next decision. Sidebar badges are reserved for Inbox because Inbox represents unprocessed incoming work. Board and Tickets do not show sidebar counts.
- Inbox count badges use black/white brand colors. Other small metadata pills may use neutral gray unless they represent a domain category.
- Keep selected states calm: visible enough to orient, never saturated enough to dominate the work surface.

### Typography

- Font stack: `Noto Sans SC`, Inter/system UI fallback.
- Letter spacing is `0` for normal UI text. Avoid negative letter spacing.
- Dashboard headings: 16-22px, 650-760 weight.
- Body copy: 13px, 1.55 line-height.
- Dense labels and metadata: 11-12px, 600-700 weight.
- Monospace only for IDs, paths, hashes, and code-like data.

### Shape And Spacing

- Default radius: 8px.
- Pills: full radius.
- Compact controls: 34-42px height.
- Page padding: 24px desktop, 12px mobile.
- Section/card gaps: 12-16px.
- Do not nest cards inside cards unless the inner card is a repeated data object.
- Use rounded boxes, not capsules, for actionable controls, toolbar buttons, run headers, form-adjacent metadata, and any element that must align with a row/grid. These controls should read as stable rectangular tools, usually 6-8px radius.
- Use capsules only when the shape itself communicates badge/tag semantics: short filters, counts, categories, and lightweight status labels that are not competing with row controls. Do not use capsule styling to decorate dense operational metadata.

### Box Model And Control Grammar

Dense editor UI must start from a stable box model. Most visual bugs in workbench surfaces come from controls fighting their containers, not from colors.

- Form controls in dense components use `box-sizing: border-box`, `width: 100%`, and `min-width: 0`.
- Grid and flex children that contain user text, IDs, chips, or selects also need `min-width: 0`.
- Text inside compact controls uses `overflow: hidden`, `text-overflow: ellipsis`, and `white-space: nowrap` unless multi-line editing is explicitly intended.
- Fixed affordances such as icon buttons, delete buttons, handles, and pin controls should have stable dimensions. Do not let labels or dynamic text change their size.
- Selects, inputs, code chips, and icon buttons in the same row should share a deliberate height. Avoid mixing browser-default heights with custom pills.
- Use named cell classes for complex rows. Avoid relying on `nth-of-type`, DOM order, or broad descendant selectors for layout-critical behavior.
- Use `minmax(0, 1fr)` when a column must shrink. Use a fixed width only for small affordances such as a 28-32px icon button.
- If a dense row cannot remain readable below a practical minimum width, preserve the row grammar and allow section-level horizontal scrolling. Do not squash fields until the content becomes meaningless.

### Icons

- Use `lucide-vue-next` for generic UI icons.
- Default icon size: 16-18px.
- Stroke width: 2.
- Icons should support text labels, not replace unclear actions unless the control has a tooltip or accessible label.
- Icon-and-text buttons must optically center both parts: use `inline-flex`, `align-items: center`, normalized `line-height`, and `display: block` on the SVG so the icon is not aligned by the text baseline.
- Keep custom SVG only for domain-specific canvases such as the dependency graph.
- App/brand icons must survive sidebar scale. Prefer one strong symbol with generous negative space over a detailed monogram. Test at 16-40px before accepting.
- Blackboard's own project badge and app mark are a special case: they follow black/white theme inversion instead of the deterministic project color palette.

## Layout Patterns

### Feature Families

When building a new feature surface, start from the nearest existing page family instead of inventing a fresh chrome.

- Reuse the closest established shell, header, toolbar, card, row, and badge grammar from Inbox, Tickets, Task Graphs, Agents, Wiki, or Settings.
- Treat a new surface as a variation inside the existing Blackboard system, not as a new visual product line.
- If a feature needs a new layout, keep the change limited to the smallest necessary hierarchy shift. Do not introduce a new color language, spacing scale, or control shape unless the product problem truly requires it.
- List/detail surfaces should stay close to the shared workbench stack. Catalog/editor surfaces should stay close to the existing split-pane or inspector patterns. Navigation surfaces should stay close to the established sidebar and workspace header grammar.
- A new feature should change information architecture first. Visual novelty is allowed only when it helps the user decide faster or reduces ambiguity.

### Shell

- Persistent left sidebar, sticky topbar, scrollable main workspace.
- Sidebar items should have enough spacing for the icon, label, and count badge to breathe.
- Topbar controls stay stable across Chinese/English i18n changes.
- Workspace navigation order is Inbox, Tickets, Agents, Wiki. Settings stays in the System group.
- The sidebar is a decision surface, not a statistics dashboard. Avoid adding counts unless they represent actionable incoming work.

### Workspaces

- Inbox: first stop for unprocessed handoff notes or incoming work. It is the only workspace sidebar item with an incoming-work count.
- Tickets: the single project execution entry. It owns Kanban, Graph, and List subviews; switching views must preserve lane/search context and feel like changing perspective on the same ticket set.
- Kanban: project execution state, organized by lane and status. Do not duplicate its open-ticket count in the sidebar.
- Graph: dependency map for understanding prerequisites, parallel branches, and blocking chains. Header explains the graph's value; toolbar explains controls and operation recipes.
- List: table/list style for filtering and opening detail. It is a durable project record, so it should not advertise total ticket count in the sidebar.
- Agents: assignment overview and agent workload. Active agent rows and hero panels use neutral brand selection, not teal.
- Wiki: project knowledge and architecture documents. The file tree, Markdown body, code blocks, tables, Mermaid diagrams, and TOC must inherit the shared Markdown renderer theme and i18n behavior rather than relying on page-local patches.
- Settings: copy focuses on sync purpose and status, not local implementation paths. Connector rows may show paths as data fields, but selected rows and sync actions stay visually neutral.

### Header Actions

Workspace headers are orientation surfaces first and action surfaces second. They should explain where the user is, then expose only the actions that belong to that workspace.

- **Shared header layout:** Persistent workspace headers (Inbox, Tickets view switch + lane controls, Agents, Task Graphs, Wiki, Settings) use one pattern: `<header class="bb-workspace-head">` with a title block `<div class="bb-workspace-head-main">` (contains `h2` plus optional `p` subtitle only) and an optional right slot `<div class="bb-workspace-head-actions">` for workspace-scoped buttons or status text. Do not put action button label styling on the whole header; keep typography scoped to `bb-workspace-head-main`.
- **Tickets toolbar:** Kanban / Graph / List use the same `bb-workspace-head` chrome with an additional class `bb-ticket-viewbar` on the same element for the tab strip + lane row (no duplicate title row above the strip).
- **Density:** Prefer compact vertical padding on these headers so the scrollable work surface starts higher; reserve taller bands for hero or empty-state content, not for routine orientation.
- **Chrome background:** Main-column sticky bands (`bb-workspace-topbar`, `bb-workspace-head`, and the project `bb-nav` bar) share `var(--bb-workbench-chrome-bg)` so they read as one shell—not pure `var(--bb-surface)` on canvas.

- The sticky topbar owns global controls such as locale, theme, lane management, and whole-project refresh. Do not duplicate those controls inside a workspace header unless the local action has a visibly narrower scope.
- Workspace header actions should be few, specific, and aligned to shared primitives such as `BbActionGroup` and `BbButton`. A primary creation action is acceptable; repeated global refresh is usually noise.
- Keep the title/subtitle block and action slot as separate layout areas. Typography selectors for header copy must target the title block, not every descendant `span` or `p`.
- Treat button label spans as part of the control, not as header copy. If an action slot needs status text, give that text its own class and box model.
- When a header button appears misaligned, check parent selector leakage before nudging icon positions. A broad selector can make the local button wrong while the same primitive is correct in the global topbar.

### Content Views

Content workspace pages use a two-part structure: a compact header above a business-owned body.

- The top band is a compact `bb-workspace-head`. It owns orientation: title, short purpose copy, and a small right-side count or action when that helps the current decision.
- Do not add a second routine toolbar below the compact header just to repeat counts, subtitles, or low-value hints. Merge that information into the header action slot or the body itself.
- The body is owned by the business surface. It may be split-pane, list/detail, table, graph canvas, document reader, or editor layout as long as it follows Blackboard density, spacing, and no-card-nesting rules.
- Body shells should use one clear rounded outer container when a framed work surface is useful. Inner regions should separate with spacing or hairlines, not stacked decorative cards.
- Markdown-first readers should let the document title occupy the reader's top line. Hide decorative heading anchors when they disrupt alignment, and keep the first document title aligned with adjacent body columns.
- Inbox full view is the current pilot for this pattern: compact workspace header with count in the right slot, then a business body with a note list on the left and Markdown reader on the right.

### Items, Cards, And Rows

- Items are single-line navigation or selection entries, not content cards.
- Item content is title-only. An item may include one fixed affordance such as a chevron, checkbox, icon, or selected mark, but it should not carry a second text channel.
- Do not place metadata, timestamps, source labels, excerpts, descriptions, badges, actions, or multi-line summaries inside an item. If that information is required, the component is a card, table row, or detail panel.
- Item height follows compact row grammar, usually 32-36px. Titles use the normal UI font, one-line ellipsis, and no monospace unless the title itself is an ID or path.
- In list/detail layouts, the left pane should use title-only items for selection. The detail pane, reader header, card, or table owns source, time, excerpts, attachments, actions, and structured content.
- Use the shared `BbObjectItem` / `bb-object-list` grammar for left-pane object selection before adding local item CSS. Local pages may provide icons or one fixed affordance, but not a second text channel.
- If a left-pane object needs delete, run, favorite, or move controls, keep those controls as adjacent row actions. The selectable item inside that row remains title-only.
- Cards represent real objects: tickets, agents, connectors, notes, graph nodes.
- Cards may contain multiple fields, status, metadata, body snippets, and actions, but they still need one clear object boundary.
- Section containers are light surfaces with hairline borders, not decorative cards.
- Use subtle hover states and focus outlines. Avoid heavy shadows.
- Do not use card nesting as a default way to create hierarchy. For operational pages, use rows, bands, split panes, and restrained bordered surfaces.
- Repeated rows should keep stable dimensions when badges, counts, or selected states appear.

### Modal Cards

Modal cards are for focused creation, confirmation, or short form workflows. They should feel like a calm interruption, not a page inside a page.

- Use a fixed body-level overlay via `Teleport` when the interaction should temporarily own focus.
- Use one centered card with 8px radius, a warm hairline border, `var(--bb-surface)`, and `var(--bb-shadow-popover)`.
- Modal structure is header, fields/body, optional inline error, footer. Header contains title, short purpose copy, and a close icon button.
- Footer actions align right. Secondary action stays neutral; primary action uses the Blackboard black/white pair unless the action has destructive semantics.
- Keep creation forms out of persistent toolbars when they need user attention. Put the trigger in the toolbar, then collect fields in the modal card.
- Modal form controls follow dense box rules: `box-sizing: border-box`, `width: 100%`, `min-width: 0`, stable height, and scoped ownership.
- Backdrop click and Escape may close non-destructive creation forms, but not while a submit is in progress.
- Errors should appear inside the modal near the form, not as a distant toast, when the user can fix the input immediately.

### Dense Editor Rows

Dense editor rows are the default grammar for structured configuration data. They should feel like small database rows: compact, predictable, and readable.

- One logical variable, binding, rule, or input equals one row. Do not pack multiple logical objects into one line to save vertical space.
- A row should expose the fields needed to understand and edit that object without making the user hunt across adjacent rows.
- Prefer a cell grammar: label above control, consistent gaps, and aligned control baselines.
- Keep destructive actions in a fixed trailing icon button. Do not place them between text fields or after a variable-length list of chips.
- Supporting actions such as "bind to input" or "insert reference" should sit on a secondary wrapped line inside the same row when horizontal space is constrained.
- Repeated chips should wrap inside their own action area. Do not let chips compete with primary text inputs for the same row width.
- A row may become taller to preserve clarity. A little vertical growth is better than horizontal crowding in a productivity tool.
- Labels in dense rows are part of the UI grammar. Use 10-11px muted labels rather than placeholders as the only explanation.

### Component Ownership And Reuse

The recurring failure mode in dense workbench UI is not a missing color token; it is unclear ownership. Parent containers, row components, and field controls must not all try to solve layout at once.

- Shared UI primitives live in `src/components/common` and are the first stop for recurring chrome. Use `BbButton`, `BbField`, `BbDialog`, `BbStatusPill`, `BbEmptyState`, `BbInlineAlert`, and `BbObjectItem` before adding page-local button, form, modal, status, empty, alert, or item CSS.
- Text-like actions use `BbInlineAction`; do not keep page-local link buttons such as `view all`, `open`, `load`, or `analytics` styled through feature-specific classes.
- Dense editor add/remove controls use `BbButton` with `size="mini"` or `size="sm"`. Page-local CSS may place the button in a grid cell, but it must not redefine the button skin, hover state, icon size, or disabled state.
- Boolean setting rows use `BbCheckboxField`. Do not create local `*-checkbox-row` classes for routine checkboxes; local components may only control placement around the shared checkbox field.
- Reference chips use `BbRefChip`. This includes read-only references such as `{{inputs.intent}}` and clickable binding shortcuts. Do not restyle reference chips in each Task Graph form.
- Binary or small option switches use `BbSegmentedControl`. Do not build local segmented buttons with ad hoc active classes.
- Summary toggles that expose a compact title plus secondary count/state use `BbSummaryChip`. This includes canvas-adjacent configuration toggles such as graph inputs/settings; do not duplicate active chip skins in editor and preview panels.
- Dense configuration rows use `BbDenseRow` plus `bb-dense-cell`, `bb-dense-cell-label`, and `bb-dense-control`. Business components may declare columns and grid areas, but row chrome, label typography, and control sizing stay in the shared primitive grammar.
- Label/value metadata uses `BbInfoGrid` and `BbInfoItem`. Use `cards` for compact fact cells and `rows` for diagnostics/popovers/document metadata; tune columns, density, mono text, and truncation through props or CSS variables on the parent. Do not restyle `dt/dd` cells locally for routine metadata. BDD story bodies such as Given/When/Then may keep a domain-specific structure because they are content, not metadata.
- Reusable section headers use `BbSectionHeader`. This covers title/count/action rows inside tickets, inbox JSON documents, and agent workbench sections. Business components may provide the action buttons, but title typography, count pill, divider, and action alignment belong to the shared primitive.
- Search/action/count toolbars use `BbToolbar`. Use `variant="bar"` for sticky workspace topbars and `variant="inline"` for action groups inside workspace headers. Business components may supply search inputs, filters, or buttons, but toolbar spacing, wrapping, sticky chrome, and action alignment stay in the primitive.
- Repeated button clusters use `BbActionGroup`; icon-only commands use `BbIconCommand` or `BbButton icon-only`. Business components may choose the action order and disabled state, but they must not redefine button skin, hover, focus, gap, or icon sizing through page-local classes.
- Primitive styling is centralized in `src/styles.css` under the shared `bb-*` grammar. Legacy selectors may be mapped there during migration, but they are compatibility shims. When a primitive needs a visual change, update the primitive/component-level grammar once instead of editing each business component.
- Business components own placement and product-specific structure only: grid, section order, object data, and domain interactions. They should not redefine border radius, button height, hover color, status pill color, field padding, dialog chrome, or empty-state typography for standard controls.
- Scoped CSS must not broadly target `button`, `input`, `select`, `textarea`, or status/empty descendants when a shared primitive is present. If an old selector still needs to coexist during migration, exclude shared primitives explicitly, for example `button:not(.bb-button)`.
- Prefer CSS variables on the parent to tune density or width of shared primitives. Do not fork a new local class because one page wants a 30px button or a narrower dialog.
- A parent component owns placement: which bands appear, their order, and how they sit beside the canvas or shell. It does not own the internal row grammar of a child component.
- A field-heavy child component owns its own rows, cells, controls, action buttons, and overflow behavior. Keep these styles scoped to the child root.
- Avoid broad selectors such as `.panel input`, `.node-card select`, or `.editor textarea` when the panel contains reusable child components. These selectors easily leak height, padding, and overflow assumptions into a component that already has a tighter grammar.
- When the same row pattern appears twice, extract it before polishing the third copy. Graph inputs, run inputs, prompt variables, and sub-pipeline bindings are all variations of the same dense field-row grammar.
- Place display/coercion logic beside the component that owns the field. For example, number/json/boolean input display and parsing belongs with the input row component, not in a page container that merely hosts it.
- Put domain helpers in domain modules, not view components. Pin colors, pin projection, render-kind selection, status visibility, dependency parsing, and slug generation should be shared helpers when more than one surface depends on them.
- Keep public import facades stable during refactors. If existing code imports from a broad domain module, move internals behind it and re-export first; update call sites only when there is a product reason.
- A reusable dense component should expose data and intent through props/events, not through a parent reaching into its DOM shape.
- Component names should describe the product object they render, not the CSS technique. Prefer `TaskGraphBindingList` over names that encode grid/flex details.
- The smaller component should still be complete. Do not extract a template while leaving its critical box model or parsing helpers in the original file.

### Recurring Small Pitfalls

These issues have appeared repeatedly in Task Graph, Wiki, Markdown, and Board surfaces. Treat them as code-review smells.

- Text fields overflow because the grid item lacks `min-width: 0`, even when the input itself has `width: 100%`.
- Compact rows drift because inputs, selects, chips, and icon buttons use different implicit browser heights.
- Reference chips and IDs look harmless with short fixtures, then break with `{{inputs.long-name}}`, long ticket IDs, or Chinese labels.
- Delete buttons shift when they are placed after variable-length chips instead of in a fixed trailing column.
- Icon-and-text buttons look "almost right" while still feeling off when SVGs keep their inline baseline box. Normalize the shared button primitive instead of nudging individual icons.
- Header copy selectors can quietly break action controls. Scope heading/subtitle styles to the title block, not every `span` inside a header, because action buttons also use spans for labels.
- Half-width form panels look tidy at first, then fail as soon as each side contains multi-field rows. Stack dense configuration bands unless both panels are genuinely sparse.
- Page-level CSS fixes make one screenshot pass but create side effects in child components. Prefer moving the box model into the component that owns the repeated pattern.
- DOM inspection can confirm structure, but it cannot confirm spatial quality. Use the actual running page when judging canvas overlays, inspector width, row density, and overflow.
- An item that grows a timestamp, excerpt, badge stack, secondary line, or inline action bar is usually a card disguised as an item. Rename it or simplify it before polishing CSS.
- Helper duplication is easy to miss in large components. If a formatter, slugifier, parser, or type-color map has to be kept "the same as" another file, it should usually be shared.
- Large files should be split along product responsibilities: catalog, editor, run inputs, binding list, renderer helpers, and domain transforms. Do not split only by line count.
- Refactors in active workbench surfaces should be small and closed: one boundary, one build, one handoff. This keeps side effects visible while multiple agents are editing nearby files.

### Graph Workbench

The Task Graph editor is a production work surface: configuration, canvas manipulation, and node inspection all compete for attention. Its layout should make the current editing task obvious.

- Treat the canvas as the primary work area. Tool panels and inspectors should support it without visually overwhelming it.
- Use full-width configuration bands when form fields are dense. Do not split two form-heavy panels into half-width columns if either side contains multi-field rows.
- For graph-level inputs, one pipeline input is one full row. The canonical order is ID, label, type, default value, reference, trailing action.
- For node bindings such as `prompt_vars` and `input_bindings`, one binding is one row. The canonical order is key, value, trailing action, then optional wrapped reference chips.
- Use the shared dense components for these patterns. The editor, detail preview, and node inspector should not each reinvent input rows or binding rows.
- LLM toolkit selectors use `BbSectionHeader` for the section title and one `BbDenseRow` per toolkit option. Keep the checkbox as the primary control and the description as muted row support text; do not restyle toolkit rows as local cards.
- Catalog sidebars should keep one card density across list, preview, and detail routes. Route-specific classes may own height, scrolling, or embedded-panel behavior, but not a different card size unless the product intentionally changes the information hierarchy.
- Catalog cards separate identity, state, and actions. Put editable/readonly state near the title, not in the action row. Actions that operate on the selected graph belong in the preview header beside Run; left catalog cards should stay light and avoid rows of repeated icon-only controls.
- Repeated catalog run affordances should be compact status squares aligned with the leading object icon. Use icon state for idle/running/succeeded/failed and keep verbose run labels in tooltips or the selected preview action area.
- Overlay inspectors must be able to scroll internally, but their rows should still be readable at the inspector's fixed width.
- Node palette, validation state, and inspector sections should be compact bands, not decorative cards stacked inside a card.
- Canvas nodes, handles, and edges carry domain semantics. Controls around them should stay neutral and quiet so graph structure remains the focus.
- When a component is edited inside the graph workbench, verify it in the actual graph page with the inspector open. DOM inspection alone is not enough for spatial issues.

## Content Guidelines

- Prefer purpose-first copy: "Visualizes ticket dependencies..." over "Drag nodes...".
- Put operation recipes in toolbar hints, empty states, or contextual tooltips.
- Avoid "how this is implemented" language unless the user is in a diagnostics view.
- Keep Chinese and English i18n paired whenever copy changes.
- Shared UI modules should expose i18n hooks instead of hardcoded English labels. For MarkdownRenderer / TableOfContents, labels such as copy-code, copied, Mermaid loading/error, lightbox controls, and TOC title belong in `src/ui/markdown` with zh/en defaults.
- Avoid absolute local paths in product summaries. If a path is shown, make it a deliberate data field or copy-path affordance.
- Brand copy is intentional: `BLACKBOARD` is the uppercase mark, with `协作工作台` / `Cowork Desk` as the lightweight product subtitle.
- Avoid over-explaining obvious UI mechanics in page headers. Headers say what the surface is for; toolbars say how to operate it.

## Implementation Checklist

Before finishing UI work:

- Check the page at desktop and mobile widths.
- Confirm text does not overflow buttons, cards, or toolbar controls.
- Confirm icon-and-text buttons are vertically centered at the actual rendered size, including both global toolbar actions and local workspace actions.
- Confirm header typography selectors do not leak into action slots, modal footers, or toolbar buttons.
- Confirm form controls use `border-box`, `min-width: 0`, and stable heights in dense rows.
- Confirm each variable, binding, rule, or input row represents one logical object.
- Confirm chips and secondary actions wrap in their own area instead of squeezing primary inputs.
- Confirm reusable child components own their internal form-control CSS; parent containers should not style their descendants' inputs/selects/textareas.
- Confirm repeated formatters, parsers, slugifiers, render-kind selectors, and domain maps live in shared helpers instead of page components.
- Confirm public module facades remain stable when internals are moved, unless the refactor intentionally changes the import contract.
- Confirm Chinese and English labels fit without shifting important controls.
- Check both light and dark mode. Compare whether the same objects are emphasized in both themes.
- Inspect sidebar, topbar, Inbox, Tickets (Kanban / Graph / List), Agents, Wiki, and Settings when a change touches shared tokens or selection styles.
- For graph changes, verify node tinting, edge contrast, pin rings, canvas framing, inspector layout, and dense configuration rows in both themes.
- For Task Graph editor changes, use the running app or Computer Use to inspect the actual graph page with node inspector content visible.
- For wiki or ticket-detail Markdown changes, verify headings, links, inline code, code blocks, tables, blockquotes, Mermaid diagrams, lightbox controls, and TOC labels in both themes and both locales.
- For brand or icon changes, verify the result in the sidebar at actual displayed size and in both themes.
- For count badges, confirm the number is actionable and not just decorative telemetry.
- Run `npm run build --prefix bb_web`.
- If Blackboard tickets or lanes changed, also run `python3 scripts/check_ticket_ids.py --project blackboard` and `qmd embed`.
