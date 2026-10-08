//! Claude Code / Codex CLI Design System theme implementation.
//!
//! Conforms strictly to the visual specifications defined in `DESIGN.md`.

use ratatui::style::Color;
use ratatui::symbols::border::Set as BorderSet;

/// Terminal dark background (`#1a1a1a`).
pub const COLOR_BG: Color = Color::Rgb(26, 26, 26);

/// Default foreground / pure white AI responses (`#ffffff`).
pub const COLOR_FG: Color = Color::Rgb(255, 255, 255);

/// Primary terracotta brand accent (`#d77757`).
pub const COLOR_PRIMARY: Color = Color::Rgb(215, 119, 87);

/// Lighter terracotta for shimmer animation (`#eb9f7f`).
pub const COLOR_CLAUDE_SHIMMER: Color = Color::Rgb(235, 159, 127);

/// Hot pink bash & tool execution border (`#fd5db1`).
pub const COLOR_SECONDARY: Color = Color::Rgb(253, 93, 177);

/// Background fill for tool & bash output (`rgb(65, 60, 65)`).
pub const COLOR_TOOL_BG: Color = Color::Rgb(65, 60, 65);

/// Lavender-blue permission dialogs and accents (`#b1b9f9`).
pub const COLOR_LAVENDER: Color = Color::Rgb(177, 185, 249);

/// Purple auto-accept / YOLO mode (`#af87ff`).
pub const COLOR_AUTO_ACCEPT: Color = Color::Rgb(175, 135, 255);

/// Green completion / success indicator (`#4eba65`).
pub const COLOR_SUCCESS: Color = Color::Rgb(78, 186, 101);

/// Amber / gold warning and caution (`#ffc107`).
pub const COLOR_WARNING: Color = Color::Rgb(255, 193, 7);

/// Soft red-pink errors (`#ff6b80`).
pub const COLOR_ERROR: Color = Color::Rgb(255, 107, 128);

/// Muted gray for inactive elements and input borders (`#888888`).
pub const COLOR_MUTED: Color = Color::Rgb(136, 136, 136);

/// Light gray for shimmering input borders (`#a6a6a6`).
pub const COLOR_MUTED_SHIMMER: Color = Color::Rgb(166, 166, 166);

/// Dark gray for subtle dividers (`#505050`).
pub const COLOR_SUBTLE: Color = Color::Rgb(80, 80, 80);

/// Surface background for user message cards (`#373737`).
pub const COLOR_SURFACE: Color = Color::Rgb(55, 55, 55);

/// Diff added line background tint (`#225c2b`).
pub const COLOR_DIFF_ADDED_BG: Color = Color::Rgb(34, 92, 43);

/// Diff removed line background tint (`#7a2936`).
pub const COLOR_DIFF_REMOVED_BG: Color = Color::Rgb(122, 41, 54);

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
