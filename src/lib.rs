pub mod animation;
pub mod app;
pub mod border;
pub mod components;
pub mod focus;
pub mod keybindings;
pub mod preset;
pub mod scroll;
pub mod theme;
pub mod ui;

pub use animation::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, Easing, ResolvedAnimationSpec,
    ScrollAnimator, TickResult, Tween, lerp_color,
};
pub use app::TuicoreApp;
pub use border::{BorderChars, border_chars, border_set};
pub use components::{
    ActivationMode, CellContext, CheckState, Column, DataView, DataViewEvent, DataViewOutcome,
    DataViewPagination, DataViewSort, DataViewTypedEvent, InputOutcome, List, ListOutcome, Panel,
    PanelVariant, SelectionGlyphs, SelectionMode, SelectionPropagation, SelectionTrigger,
    SortDirection, Spinner, Tab, Tabs, TextInput, TextareaInput, TreeAdapter, TreeGlyphs,
};
pub use focus::{
    FocusChain, FocusDirection, FocusOutcome, FocusRouter, FocusRouterError, FocusWrap,
};
pub use keybindings::{
    DataViewKeyBindings, FocusKeyBindings, KeyBindings, KeySpec, TabsKeyBindings,
};
pub use preset::{BorderKind, DataViewPreset, Preset, TabsPreset, TabsVariant};
pub use scroll::{
    ScrollAxes, ScrollBehavior, ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset,
    ScrollOutcome, ScrollPreset, ScrollSize, ScrollState, ScrollbarConfig, ScrollbarGutter,
    ScrollbarStyle, ScrollbarVisibility, line_width, paragraph_scroll, text_size,
};
pub use theme::{Theme, ThemeName};
pub use ui::{
    animation_settings, animation_tick_subscription, animation_tick_subscriptions, init,
    init_from_dir, keybindings, preset, set_keybindings, set_preset, set_theme, theme,
};
