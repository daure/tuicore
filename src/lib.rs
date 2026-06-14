pub mod animation;
pub mod border;
pub mod components;
pub mod keybindings;
pub mod preset;
pub mod scroll;
pub mod theme;
pub mod ui;

pub use animation::{
    Animated, AnimationSettings, AnimationSpec, Easing, ResolvedAnimationSpec, ScrollAnimator,
    TickResult, Tween,
};
pub use border::{BorderChars, border_chars, border_set};
pub use components::{Panel, Tab, Tabs};
pub use keybindings::{KeyBindings, KeySpec, TabsKeyBindings};
pub use preset::{BorderKind, Preset, TabsPreset, TabsVariant};
pub use scroll::{
    ScrollAxes, ScrollBehavior, ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset,
    ScrollOutcome, ScrollPreset, ScrollSize, ScrollState, ScrollbarConfig, ScrollbarGutter,
    ScrollbarVisibility, line_width, paragraph_scroll, text_size,
};
pub use theme::{Theme, ThemeName};
pub use ui::{
    animation_settings, init, init_from_dir, keybindings, preset, set_keybindings, set_preset,
    set_theme, theme,
};
