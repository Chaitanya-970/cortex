//! Codex CLI & Claude Code TUI Design System theme implementation.
//!
//! Conforms strictly to the visual specifications defined in `DESIGN.md`.

use ratatui::style::Color;
use ratatui::symbols::border::Set as BorderSet;

/// Visual theme mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Blue theme: Electric Blue (#3b82f6), Sky Cyan (#38bdf8), Ice Blue (#93c5fd).
    #[default]
    Blue,
    /// Codex CLI & Claude Code theme: Terracotta (#d77757), Hot Pink (#fd5db1), Lavender (#b1b9f9).
    Codex,
    /// Gemini CLI theme: Electric Blue (#4285f4), Sparkle Violet (#a855f7), Sky Cyan (#38bdf8), ✦ sparkle star.
    Gemini,
}

// =========================================================================
// Blue Theme Color Palette
// =========================================================================

/// Terminal dark background (`#0f172a` deep midnight slate navy).
pub const COLOR_BG: Color = Color::Rgb(15, 23, 42);

/// Default foreground / pure white AI responses (`#ffffff`).
pub const COLOR_FG: Color = Color::Rgb(255, 255, 255);

/// Primary Electric Blue brand accent (`#3b82f6`).
pub const COLOR_PRIMARY: Color = Color::Rgb(59, 130, 246);

/// Sky Cyan bash & tool execution border (`#38bdf8`).
pub const COLOR_SECONDARY: Color = Color::Rgb(56, 189, 248);

/// Royal / Ice Blue dialogs and accents (`#93c5fd`).
pub const COLOR_ACCENT: Color = Color::Rgb(147, 197, 253);

/// Lighter electric blue for shimmer animation (`#bfdbfe`).
pub const COLOR_CLAUDE_SHIMMER: Color = Color::Rgb(191, 219, 254);

/// Compatibility alias for Gemini shimmer.
pub const COLOR_GEMINI_SHIMMER: Color = COLOR_CLAUDE_SHIMMER;

/// Background fill for tool & bash output (deep slate navy `#1e293b`).
pub const COLOR_TOOL_BG: Color = Color::Rgb(30, 41, 59);

/// Lavender-blue accents (`#a5b4fc`).
pub const COLOR_LAVENDER: Color = Color::Rgb(165, 180, 252);

/// Blue-violet auto-accept / YOLO mode (`#818cf8`).
pub const COLOR_AUTO_ACCEPT: Color = Color::Rgb(129, 140, 248);

/// Green completion / success indicator (`#34d399`).
pub const COLOR_SUCCESS: Color = Color::Rgb(52, 211, 153);

/// Amber / gold warning and caution (`#fbbf24`).
pub const COLOR_WARNING: Color = Color::Rgb(251, 191, 36);

/// Soft red errors (`#f87171`).
pub const COLOR_ERROR: Color = Color::Rgb(248, 113, 113);

/// Muted slate gray for inactive elements and input borders (`#64748b`).
pub const COLOR_MUTED: Color = Color::Rgb(100, 116, 139);

/// Light slate for shimmering input borders (`#94a3b8`).
pub const COLOR_MUTED_SHIMMER: Color = Color::Rgb(148, 163, 184);

/// Dark slate navy for subtle dividers (`#334155`).
pub const COLOR_SUBTLE: Color = Color::Rgb(51, 65, 85);

/// Surface background for user message cards (`#1e293b`).
pub const COLOR_SURFACE: Color = Color::Rgb(30, 41, 59);

/// Diff added line background tint (`#064e3b`).
pub const COLOR_DIFF_ADDED_BG: Color = Color::Rgb(6, 78, 59);

/// Diff removed line background tint (`#7f1d1d`).
pub const COLOR_DIFF_REMOVED_BG: Color = Color::Rgb(127, 29, 29);

/// Reverse-mirror thinking spinner symbols per DESIGN.md:
/// `· → ✢ → ✳ → ✶ → ✻ → ✽ → ✻ → ✶ → ✳ → ✢ → · ...`
pub const SPINNER_FRAMES: &[&str] = &["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];

/// Curated whimsical thinking verbs per DESIGN.md.
pub const WHIMSICAL_VERBS: &[&str] = &[
    "Percolating...",
    "Cogitating...",
    "Moonwalking...",
    "Shenaniganing...",
    "Ruminating...",
    "Noodling...",
    "Baking...",
    "Fermenting...",
    "Transmuting...",
    "Pondering...",
    "Brewing...",
    "Synthesizing...",
    "Conjuring...",
    "Deliberating...",
    "Manifesting...",
    "Vibing...",
    "Simmering...",
    "Decocting...",
    "Untangling...",
    "Orchestrating...",
    "Harmonizing...",
    "Daydreaming...",
    "Polishing...",
    "Refactoring...",
    "Calculating...",
    "Optimizing...",
];

/// Dashed ASCII border set for input box per DESIGN.md (`- - | -`).
pub const DASHED_INPUT_SET: BorderSet = BorderSet {
    top_left: "-",
    top_right: "-",
    bottom_left: "-",
    bottom_right: "-",
    vertical_left: "|",
    vertical_right: "|",
    horizontal_top: "-",
    horizontal_bottom: "-",
};

/// Signature prompt glyph: `> ` for Codex CLI.
pub fn prompt_glyph() -> &'static str {
    "> "
}

/// Get active spinner frame glyph for current animation tick.
pub fn spinner_frame(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

/// Shimmer terracotta color for thinking indicator.
pub fn thinking_shimmer_color(tick: usize) -> Color {
    if (tick / 2) % 2 == 0 {
        COLOR_PRIMARY
    } else {
        COLOR_CLAUDE_SHIMMER
    }
}

/// Shimmer gray color for dashed input box border.
pub fn input_shimmer_color(tick: usize) -> Color {
    if (tick / 3) % 2 == 0 {
        COLOR_MUTED
    } else {
        COLOR_MUTED_SHIMMER
    }
}

/// Get whimsical thinking verb for given index.
pub fn whimsical_verb(index: usize) -> &'static str {
    WHIMSICAL_VERBS[index % WHIMSICAL_VERBS.len()]
}
