# Blackboard TDesign Integration

This document is the implementation companion to `DESIGN.md`. `DESIGN.md` owns Blackboard's product posture, visual system, density, and semantics. This file owns how `bb_web` uses TDesign Vue Next as a component primitive layer.

TDesign is an implementation substrate, not the design source of truth. Use it for mature behavior, accessibility, portals, keyboard handling, and component state; keep Blackboard's visual grammar in local wrappers and `styles.css`.

## Baseline

- Package: `tdesign-vue-next` in `bb_web/package.json`.
- Framework target: TDesign Vue Next, meaning Vue 3. Do not use Vue 2 `tdesign-vue` APIs.
- Global TDesign style is imported once in `src/main.ts` via `tdesign-vue-next/es/style/index.css`.
- Blackboard theme state lives in `src/theme.ts`. It sets both `data-theme` for Blackboard CSS and `theme-mode` for TDesign theme behavior.
- Prefer direct ESM imports from `tdesign-vue-next/es/<component>` or `tdesign-vue-next/es/<plugin>`.

## Theme Bridge

Blackboard's theme color is the black/white brand pair, not TDesign's default blue. TDesign exposes component theme colors through CSS variables such as `--td-brand-color-*` under `:root[theme-mode='light']` and `:root[theme-mode='dark']`; Blackboard overrides those variables in `src/styles.css`.

Theme state is split in `src/theme.ts`:

- `themeMode`: light or dark mode.
- `themeSkin`: preset skin id. The first preset is `blackboard`.
- `data-theme`: mirrors `themeMode` for Blackboard CSS.
- `theme-mode`: mirrors `themeMode` for TDesign.
- `data-skin`: mirrors `themeSkin` for skin-specific CSS hooks.

Skin presets are internal infrastructure for now. Do not expose a skin picker in the workspace chrome until Blackboard has more than one reviewed preset.

Blackboard theme semantics:

- `--bb-theme-primary`: the current brand foreground color used for generic active or selected product chrome.
- `--bb-theme-on-primary`: readable text/icon color on top of `--bb-theme-primary`.
- `--bb-theme-primary-soft`: subtle selected/active surface tint.
- `--bb-theme-primary-border` and `--bb-theme-primary-border-strong`: neutral brand borders for selected states.
- Domain colors remain separate: task graph run status, editable/readonly state, ticket status, lane colors, graph node colors, success/warning/error, and focus rings keep their own semantic colors.

When a component represents generic app selection or active navigation, use the `--bb-theme-*` tokens. When a component represents domain state, do not force it into black/white.

## Current Adopted Components

### Badge

Official docs: https://tdesign.tencent.com/vue-next/components/badge

Use TDesign `Badge` when a compact count should attach to an icon or nearby text.

- Sidebar count badges are reserved for actionable incoming work, currently Inbox.
- Prefer a badge on the icon over a separate trailing count pill when the goal is information compression.
- Do not show decorative telemetry counts in the sidebar. Counts must change the user's next decision.
- Keep `showZero` off unless zero is meaningful to the workflow.
- Use `max-count` for potentially large counts so narrow navigation does not expand unexpectedly.
- Style through a local wrapper such as `.bb-side-nav-badge`; avoid broad global `.t-badge` overrides.

### Dropdown

Official docs: https://tdesign.tencent.com/vue-next/components/dropdown

Use `src/components/BbDropdown.vue` for Blackboard dropdowns instead of raw TDesign `Dropdown` in product surfaces.

`BbDropdown` standardizes:

- click trigger behavior;
- Blackboard trigger shape and density;
- option rows with label, description, badge, disabled, and divider states;
- `v-model` plus `change` events;
- teleported overlay classes: `.bb-dropdown-popup` and `.bb-dropdown-panel`;
- consistent menu width and max-height behavior.

`UnifiedPopupSelect.vue` is a compatibility wrapper that delegates to `BbDropdown`. New select-like popup work should use `BbDropdown` directly unless it needs the older API.

Avoid adding new native `<select>` controls for primary app navigation, lane switching, project switching, or other repeated product dropdowns. Native selects are acceptable for small form fields where the platform control is the intended interaction.

### Notification

Official docs: https://tdesign.tencent.com/vue-next/components/notification

Use `NotifyPlugin` for global, non-blocking async operation feedback when the result is not naturally tied to one visible form field.

Current save-feedback convention:

- Success: `NotifyPlugin.success`, `duration: 4000`, close button enabled.
- Failure: `NotifyPlugin.error`, `duration: 0`, close button enabled.
- Failure content must explain the reason, the next action, and useful details. It should read like an alert, not like a disappearing toast.
- Keep placement stable at top-right unless the surrounding workflow has a strong reason to differ.
- Use a local `className` for styling teleported notifications, such as `bb-task-graph-notification`.
- Close or replace stale notifications before showing a new save result.

Use this pattern for production-tool failures where the user may need time to read and fix the issue.

### Alert And Message

Official docs:

- Alert: https://tdesign.tencent.com/vue-next/components/alert
- Message: https://tdesign.tencent.com/vue-next/components/message

Use Alert semantics for persistent in-surface states: validation summaries, blocked panels, offline states, or modal/form errors the user can fix in place.

Use Message only for low-risk, short-lived confirmation where no reading or recovery is required, such as a copied-to-clipboard acknowledgement. Do not use Message for save failure, data-loss risk, validation errors, or task graph execution failures.

## Styling Rules

- Blackboard tokens remain authoritative: canvas, surface, text, hairline, focus, semantic status colors, and brand black/white selection all come from `styles.css`.
- TDesign default blue/red styling should not leak into generic Blackboard selection states.
- Scope overrides through local wrapper classes. Prefer `.bb-dropdown-panel .t-dropdown__item` over global `.t-dropdown__item`.
- Teleported components need explicit classes because scoped component CSS will not reach the body overlay.
- Keep density aligned with `DESIGN.md`: compact controls are usually 34-42px high, cards use 8px radius or less, and repeated rows keep stable dimensions.
- Preserve accessible primitives. If a TDesign trigger is replaced by a custom button, keep button semantics, disabled state, `aria-label`, and `aria-expanded` where applicable.

## Component Selection Policy

Before adding a TDesign component:

1. Decide whether the interaction is a reusable Blackboard primitive.
2. If it is reused or visually central, create a `Bb*` wrapper in `src/components`.
3. Keep the wrapper API product-shaped, not TDesign-shaped. For example, expose `options`, `modelValue`, and `change` rather than leaking every low-level popup prop.
4. Use TDesign for behavior and state, then re-skin through Blackboard classes.
5. Add i18n labels in `src/i18n.ts` for visible copy.
6. Run `npm run build --prefix bb_web`.
7. Visually verify teleported overlays, dark mode, and narrow widths when the component affects shell UI.

## Do Not

- Do not introduce SemiDesign or another parallel component system for the same primitives.
- Do not globally install TDesign as the visual language of the app.
- Do not scatter raw TDesign `Dropdown` usage across pages when `BbDropdown` can cover the need.
- Do not hand-roll new toast, badge, or dropdown primitives unless TDesign cannot support the required behavior.
- Do not rely on auto-closing feedback for failures that require reading, recovery, or user action.
- Do not override broad `.t-*` selectors without a Blackboard wrapper scope.

## Review Checklist

For TDesign-related changes, confirm:

- imports use `tdesign-vue-next` Vue Next APIs;
- `theme-mode` still follows Blackboard light/dark state;
- Blackboard wrapper classes own visual density and color;
- teleported popups/notifications receive explicit classes;
- failure notifications persist until the user closes them;
- sidebar badges only show actionable incoming work;
- dropdowns use `BbDropdown` unless there is a documented exception;
- `npm run build --prefix bb_web` passes.
