# Codex CLI & Claude Code — TUI Design System

> The terminal-first AI coding agent experience. Based on [Claude Code](https://claude.ai/claude-code) and the Codex CLI terminal harness — warm, playful, and content-forward with signature terracotta and hot pink accents.

## 1. Theme Overview

- **Mood**: Warm, developer-first, playful, content-forward
- **Density**: Balanced — clean conversation stream, dashed ASCII framing, minimal chrome
- **Target**: Autonomous AI coding workers, terminal agent harnesses, Codex developer workflows
- **Terminal**: TrueColor recommended for terracotta shimmer (`#d77757` ↔ `#eb9f7f`), 256-color fallback supported

## 2. Color Palette

### Semantic Roles (Codex CLI / Claude Code Theme)

| Role | Hex | ANSI 256 | ANSI 16 | Usage |
|------|-----|----------|---------|-------|
| Background | `#1a1a1a` | `234` | `black` | Terminal dark background |
| Foreground | `#ffffff` | `15` | `bright white` | Default text, AI responses |
| Primary | `#d77757` | `173` | `yellow` | Terracotta — brand accent & headers |
| Secondary | `#fd5db1` | `206` | `bright magenta` | Hot pink — tool call & bash execution borders |
| Accent | `#b1b9f9` | `147` | `bright blue` | Lavender — permission dialogs & highlights |
| Success | `#4eba65` | `71` | `green` | Green — completion & verification pass |
| Warning | `#ffc107` | `220` | `yellow` | Amber / gold — caution |
| Error | `#ff6b80` | `204` | `red` | Soft red-pink — errors & failures |
| Muted | `#888888` | `245` | `bright black` | Slate gray — input borders, inactive |
| Surface | `#373737` | `237` | `black` | User message background |

### Theme-Specific Accents

| Name | Hex | Usage |
|------|-----|-------|
| Shimmer terracotta | `#eb9f7f` | Lighter terracotta for shimmer animation |
| Bash border | `#fd5db1` | Hot pink tool execution borders |
| Permission | `#b1b9f9` | Lavender-blue permission dialogs |
| Auto-accept | `#af87ff` | Purple — YOLO / auto-accept mode |
| Inactive | `#999999` | Gray — disabled elements |
| Subtle | `#505050` | Dark gray — subtle dividers |
| Diff added bg | `#225c2b` | Green tint for added lines |
| Diff removed bg | `#7a2936` | Red tint for removed lines |

## 3. Typography & ASCII Art

- **Clean Typography**: Clean monospaced terminal typography
- **Prompt Glyph**: `> ` in Primary Terracotta (`#d77757`)
- **Readiness Badge**: `● Ready` in Success Green (`#4eba65`)
- **Headers & Emphases**: Bold Terracotta (`#d77757`)
- **AI Response Text**: Crisp White (`#ffffff`) on default dark background

### Text Hierarchy

| Level | Style | Example Usage |
|-------|-------|---------------|
| H1 | BOLD + Primary (terracotta `#d77757`) | Header banner (`◈ Cortex Code`) |
| Body | Foreground (`#ffffff`) | AI response text |
| Code | Syntax highlighted with pink & cyan accents | Code blocks |
| User input | `> ` prefix on Surface bg (`#373737`) | User messages |
| Caption | Muted slate (`#888888`) | Token counts, timestamps, files indexed |
| Thinking | Primary terracotta + shimmer (`#d77757` ↔ `#eb9f7f`) | Thinking spinner & verb |

## 4. Borders & Box Drawing

### Input Box (Dashed ASCII — Signature Style)

```text
- - - - - - - - - - - - - - - -
| > your message here_          |
- - - - - - - - - - - - - - - -
```

**No Unicode box-drawing for input** — plain ASCII dashed lines (`-` horizontal, `|` vertical). Border is Muted gray with shimmer between `#888888` and `#a6a6a6`.

### Tool Call Border (Hot Pink)

```text
┌─ Bash: npm test ──────────────┐
│                                │
│ PASS  src/app.test.ts          │
│   ✓ handles input (12ms)      │
│                                │
└────────────────────────────────┘
```

Tool and bash execution outputs use box-drawing with hot pink (`#fd5db1`) borders and `rgb(65, 60, 65)` background fill.

### Permission Dialog (Lavender)

```text
┌─ Allow Edit to src/app.ts? ───┐
│                                │
│  [Y]es  [N]o  [A]lways        │
│                                │
└────────────────────────────────┘
```

Permission prompts use lavender (`#b1b9f9`) borders with option accelerator keys highlighted.

### Parts Table

| Part | Character | Color | Usage |
|------|-----------|-------|-------|
| Input horizontal | `-` (dashed) | Muted gray (`#888888`) | Input box |
| Input vertical | `\|` | Muted gray (`#888888`) | Input box sides |
| Tool top_left | `┌` | Hot pink (`#fd5db1`) | Tool call blocks |
| Tool horizontal | `─` | Hot pink (`#fd5db1`) | Tool call blocks |
| Tool vertical | `│` | Hot pink (`#fd5db1`) | Tool call blocks |
| Tool bottom_left | `└` | Hot pink (`#fd5db1`) | Tool call blocks |
| Permission border | `┌─┐│└─┘` | Lavender (`#b1b9f9`) | Permission dialogs |

---

## 5. Components & Interactions

### User Prompt Line

```text
> fix the authentication bug
```

- Signature `> ` prefix in Terracotta (`#d77757`)
- Surface card background (`#373737`) for user message cards in history

### Thinking Indicator (Signature Feature)

```text
  ✳ Percolating... (Press Esc to cancel)
```

- Reverse-mirror thinking spinner:
  ```text
  · → ✢ → ✳ → ✶ → ✻ → ✽ → ✻ → ✶ → ✳ → ✢ → · ...
  ```
- 120ms animation interval
- Rendered in terracotta with shimmer to `#eb9f7f`
- Paired with curated whimsical verbs:
  "Percolating...", "Cogitating...", "Moonwalking...", "Shenaniganing...", "Ruminating...", "Noodling...", "Baking...", "Fermenting...", "Transmuting...", "Pondering..."

### Compact Tool Executions

```text
  grep "authenticate" src/
  ✓ 6 matches

  Reading src/auth.ts
  ✓ 87 lines

  ✎ src/auth.ts
  ✓ updated

  $ cargo test
  ✓ Tests passed
```

### Persistent Status Bar (Bottom)

```text
  gpt-4o · 12.4K tokens · $0.0400 · 3.2s · normal · /help for commands
```

- Model identifier, cumulative session tokens, cost accumulator, elapsed execution duration, effort level

---

## 6. Layout & Spacing

- **Min terminal width**: `80`
- **Ideal terminal width**: `120`
- **Padding inside tool blocks**: 1 line top/bottom, 1 char left/right
- **Gap between messages**: 1 line with subtle divider (`#505050`)
- **Indent level**: 2 spaces

---

## 7. Icons & Indicators

| Purpose | Icon | Fallback (ASCII) |
|---------|------|-------------------|
| Success | `✓` | `+` |
| Error | `✗` | `x` |
| Warning | `⚠` | `!` |
| Thinking | `· ✢ ✳ ✶ ✻ ✽` | `*` |
| Prompt | `>` | `>` |
| Ready | `●` | `*` |
| Running | `▸` | `>` |
| Bullet | `•` | `-` |

---

## 8. Alternate Themes: Gemini CLI

Cortex also provides support for the **Gemini CLI** theme via `"theme": "gemini"` in `~/.cortex/settings.json`:
- Primary: Electric Blue (`#4285f4`)
- Secondary: Sparkle Violet (`#a855f7`)
- Accent: Sky Cyan (`#38bdf8`)
- Prompt Glyph: `✦ `
- Thinking Spinner: `✦ → ✧ → ⟡ → ❖ → ⟡ → ✧`
- Reasoning Verbs: "Reasoning...", "Synthesizing...", "Analyzing...", "Formulating..."
