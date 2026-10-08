# Gemini CLI — TUI Design System

> The next-generation AI coding agent terminal experience. Inspired by [Google Gemini CLI](https://gemini.google.com) and Google AI Studio — sleek, high-velocity, and information-dense with signature Gemini Electric Blue, Sparkle Violet, and Sky Cyan accents.

## 1. Theme Overview

- **Mood**: High-velocity, sleek, futuristic, minimalist developer console
- **Density**: Compact — clean conversation stream, borderless flow, zero wasted chrome
- **Target**: Autonomous AI coding workers, terminal agent harnesses, Google Gemini developer workflows
- **Terminal**: TrueColor recommended for Gemini gradient shimmer (`#4285f4` ↔ `#a855f7`), 256-color fallback supported

## 2. Color Palette

### Semantic Roles (Gemini CLI Theme)

| Role | Hex | ANSI 256 | ANSI 16 | Usage |
|------|-----|----------|---------|-------|
| Background | `#10141d` | `234` | `black` | Deep midnight navy background |
| Foreground | `#ffffff` | `15` | `bright white` | Clean white text, AI responses |
| Primary | `#4285f4` | `75` | `bright blue` | Gemini Electric Blue — Google brand accent & prompt |
| Secondary | `#a855f7` | `135` | `bright magenta` | Gemini Sparkle Violet — secondary accents & tool borders |
| Accent | `#38bdf8` | `81` | `bright cyan` | Sky Cyan — paths, code diff highlights, tool calls |
| Success | `#34a853` | `71` | `green` | Google Green — completion / test passes |
| Warning | `#fbbc04` | `220` | `yellow` | Google Amber / Gold — caution |
| Error | `#ea4335` | `203` | `red` | Google Coral Red — errors & failures |
| Muted | `#94a3b8` | `246` | `bright black` | Slate gray — metadata, inactive elements |
| Subtle | `#334155` | `237` | `black` | Deep slate — separators |
| Surface | `#1e293b` | `236` | `black` | Elevated card background |

### Gemini Gradient Shimmer

The thinking indicator and interactive elements shimmer smoothly along the signature Gemini gradient:

$$\text{Electric Blue (\#4285f4)} \longleftrightarrow \text{Sky Cyan (\#38bdf8)} \longleftrightarrow \text{Sparkle Violet (\#a855f7)} \longleftrightarrow \text{Soft Blue (\#8ab4f8)}$$

### Diff Colors

| Name | Hex | Usage |
|------|-----|-------|
| Diff added bg | `#14532d` | Forest green tint for added lines |
| Diff removed bg | `#7f1d1d` | Dark red tint for removed lines |

---

## 3. Typography & ASCII Art

- **Clean Typography**: Clean, crisp monospaced terminal typography
- **Prompt Glyph**: `✦ ` (Gemini 4-point sparkle star)
- **Readiness Badge**: `✦ Ready` in Google Green (`#34a853`)
- **Headers & Emphases**: Bold Gemini Electric Blue (`#4285f4`)

### Text Hierarchy

| Level | Style | Example Usage |
|-------|-------|---------------|
| H1 | BOLD + Primary (Gemini Blue `#4285f4`) | Welcome header (`✦ Cortex Code`) |
| Body | Foreground (`#ffffff`) | AI response text |
| Code | Syntax highlighted with Sky Cyan (`#38bdf8`) & Violet (`#a855f7`) | Code blocks |
| User input | `✦ ` prefix in Gemini Blue | User messages |
| Caption | Muted slate (`#94a3b8`) | Token counts, timestamps, files indexed |
| Thinking | Shimmering Gemini Gradient (`#4285f4` ↔ `#a855f7`) | Reasoning indicator |

---

## 4. Borders & Box Drawing

### Tool Call Block (Gemini Cyan & Violet)

```text
┌─ Read: src/config.ts ─────────────────┐
│                                         │
│  1 │ export function parseConfig() {    │
│  2 │   const raw = readFileSync(path);  │
│  3 │   return JSON.parse(raw);          │
│                                         │
└─────────────────────────────────────────┘
```

- Rounded or thin single-line box-drawing with Gemini Sky Cyan (`#38bdf8`) or Sparkle Violet (`#a855f7`) borders
- Background fill: Midnight Slate (`#18202f`)

### Autocomplete Menu (Gemini Accent)

```text
┌── Slash Commands ──────────────────────────────────────────┐
│  /model       Select or configure AI model                 │
│  /settings    Configure ~/.cortex/settings.json            │
│  /status      Show workspace and runtime status            │
└────────────────────────────────────────────────────────────┘
```

- Border: Gemini Blue (`#4285f4`)
- Highlight: Inverted Electric Blue background with crisp white text

---

## 5. Components & Glyphs

### Prompt & Input Line

```text
✦ fix the authentication bug
```

- Signature `✦ ` prefix in Gemini Electric Blue (`#4285f4`)
- Dynamic multi-line height for multi-line entries (`Alt+Enter` or trailing `\ + Enter`)

### Thinking & Reasoning Indicator

```text
✦ Reasoning... (Ctrl+C to cancel)
```

- Spinner cycles through the signature Gemini sparkle sequence:
  ```text
  ✦ → ✧ → ⟡ → ❖ → ⟡ → ✧ → ✦ ...
  ```
- Shimmer colors oscillate through the Gemini gradient (`#4285f4` ↔ `#38bdf8` ↔ `#a855f7`)
- Paired with analytical reasoning verbs:
  "Reasoning...", "Synthesizing...", "Analyzing...", "Formulating...", "Contextualizing...", "Optimizing..."

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

- Tool commands: Sky Cyan (`#38bdf8`) and Gemini Blue (`#4285f4`)
- Success checks: Google Green `✓` (`#34a853`)

---

## 6. Alternate Themes: Claude Code

Cortex also maintains support for the classic **Claude Code** theme via `"theme": "claude"` in `~/.cortex/settings.json`:
- Primary: Terracotta (`#d77757`)
- Secondary: Hot Pink (`#fd5db1`)
- Permission: Lavender (`#b1b9f9`)
- Spinner: Reverse-mirror `· ✢ ✳ ✶ ✻ ✽`
