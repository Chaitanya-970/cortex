//! Gemini CLI / Claude Code TUI Design System theme implementation.
//!
//! Conforms strictly to the visual specifications defined in `DESIGN.md`.

use ratatui::style::Color;
use ratatui::symbols::border::Set as BorderSet;

/// Visual theme mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Gemini CLI theme: signature Gemini Electric Blue (#4285f4), Sparkle Violet (#a855f7), Sky Cyan (#38bdf8), ✦ sparkle star.
    #[default]
    Gemini,
    /// Claude Code theme: Terracotta (#d77757), Hot Pink (#fd5db1), Lavender (#b1b9f9).
    Claude,
}

// =========================================================================
// Gemini CLI Theme Color Palette
// =========================================================================

/// Terminal dark background (`#10141d` - Deep Gemini Midnight Navy).
pub const COLOR_BG: Color = Color::Rgb(16, 20, 29);

/// Default foreground / pure white AI responses (`#ffffff`).
pub const COLOR_FG: Color = Color::Rgb(255, 255, 255);

/// Primary Gemini Electric Blue brand accent (`#4285f4`).
pub const COLOR_PRIMARY: Color = Color::Rgb(66, 133, 244);

/// Gemini Sparkle Violet secondary accent (`#a855f7`).
pub const COLOR_SECONDARY: Color = Color::Rgb(168, 85, 247);

/// Gemini Sky Cyan tool and path accent (`#38bdf8`).
pub const COLOR_ACCENT: Color = Color::Rgb(56, 189, 248);

/// Soft Gemini Blue for shimmer gradient (`#8ab4f8`).
pub const COLOR_GEMINI_SHIMMER: Color = Color::Rgb(138, 180, 248);

/// Claude shimmer compatibility alias (`#8ab4f8`).
pub const COLOR_CLAUDE_SHIMMER: Color = COLOR_GEMINI_SHIMMER;

/// Background fill for tool output (`#18202f`).
pub const COLOR_TOOL_BG: Color = Color::Rgb(24, 32, 47);

/// Gemini Light Violet permission dialogs and accents (`#c084fc`).
pub const COLOR_LAVENDER: Color = Color::Rgb(192, 132, 252);

/// Purple auto-accept / YOLO mode (`#af87ff`).
pub const COLOR_AUTO_ACCEPT: Color = Color::Rgb(175, 135, 255);

/// Google Green completion / success indicator (`#34a853`).
pub const COLOR_SUCCESS: Color = Color::Rgb(52, 168, 83);

/// Google Amber warning and caution (`#fbbc04`).
pub const COLOR_WARNING: Color = Color::Rgb(251, 188, 4);

/// Google Coral Red errors (`#ea4335`).
pub const COLOR_ERROR: Color = Color::Rgb(234, 67, 53);

/// Slate gray for inactive elements and metadata (`#94a3b8`).
pub const COLOR_MUTED: Color = Color::Rgb(148, 163, 184);

/// Light slate for shimmering input borders (`#cbd5e1`).
pub const COLOR_MUTED_SHIMMER: Color = Color::Rgb(203, 213, 225);

/// Deep slate for subtle dividers (`#334155`).
pub const COLOR_SUBTLE: Color = Color::Rgb(51, 65, 85);

/// Surface background for user message cards (`#1e293b`).
pub const COLOR_SURFACE: Color = Color::Rgb(30, 41, 59);

/// Diff added line background tint (`#14532d`).
pub const COLOR_DIFF_ADDED_BG: Color = Color::Rgb(20, 83, 45);

/// Diff removed line background tint (`#7f1d1d`).
pub const COLOR_DIFF_REMOVED_BG: Color = Color::Rgb(127, 29, 29);

/// Gemini signature 4-point sparkle star spinner frames:
/// `✦ → ✧ → ⟡ → ❖ → ⟡ → ✧ → ✦ ...`
pub const SPINNER_FRAMES: &[&str] = &["✦", "✧", "⟡", "❖", "⟡", "✧"];

/// Curated Gemini reasoning and thinking verbs per DESIGN.md.
pub const WHIMSICAL_VERBS: &[&str] = &[
    "Reasoning...",
    "Synthesizing...",
    "Analyzing...",
    "Formulating...",
    "Contextualizing...",
    "Optimizing...",
    "Decomposing...",
    "Deriving...",
    "Structuring...",
    "Refining...",
    "Pondering...",
    "Orchestrating...",
    "Evaluating...",
    "Navigating...",
    "Harmonizing...",
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

/// Signature prompt glyph: `✦ ` for Gemini CLI.
pub fn prompt_glyph() -> &'static str {
    "✦ "
}

/// Get active spinner frame glyph for current animation tick.
pub fn spinner_frame(tick: usize) -> &'static str {
    SPINNER_FRAMES[tick % SPINNER_FRAMES.len()]
}

/// Shimmer color for thinking indicator (Gemini Blue ↔ Cyan ↔ Violet gradient).
pub fn thinking_shimmer_color(tick: usize) -> Color {
    match (tick / 2) % 4 {
        0 => COLOR_PRIMARY,        // Electric Blue #4285f4
        1 => COLOR_ACCENT,         // Sky Cyan #38bdf8
        2 => COLOR_SECONDARY,      // Sparkle Violet #a855f7
        _ => COLOR_GEMINI_SHIMMER, // Soft Blue #8ab4f8
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
