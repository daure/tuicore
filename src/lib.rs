pub mod animation;
pub mod border;
pub mod children;
pub mod components;
pub mod event;
pub mod focus;
pub mod keybindings;
pub mod node;
pub mod preset;
pub mod runtime;
pub mod scroll;
pub mod search;
pub mod separator;
mod spacing;
pub mod theme;
pub mod ui;

pub use animation::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, Easing, ResolvedAnimationSpec,
    ScrollAnimator, TickResult, Tween, lerp_color,
};
pub use border::{BorderChars, border_chars, border_set};
pub use children::{ChildSlot, Children, DuplicateChildKey, MissingChildKey};
pub use components::{
    ActivationMode, Button, ButtonOutcome, CellContext, CheckState, Column, CrossAlign, CrossSize,
    DataView, DataViewEvent, DataViewOutcome, DataViewPagination, DataViewSort, DataViewTypedEvent,
    Dropdown, DropdownCommitMode, DropdownOutcome, DropdownSearchMode, DropdownVariant, Flex,
    FlexItem, Grid, GridItem, GridTrack, InputOutcome, List, ListOutcome, MainAlign, Overlay,
    OverlayAnchor, OverlaySize, Panel, PanelHost, PanelTitlePosition, SelectionGlyphs,
    SelectionMode, SelectionPropagation, SelectionTrigger, SortDirection, Spinner, Split, Stack,
    StackAlign, StackItem, StackSize, Tab, Tabs, TextInput, TextareaInput, Toggle, ToggleOutcome,
    TreeAdapter, TreeGlyphs,
};
pub use event::{
    Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, TuiEvent,
    UnsupportedEvent,
};
pub use focus::{
    FocusChain, FocusDirection, FocusOutcome, FocusRouter, FocusRouterError, FocusWrap,
};
pub use keybindings::{
    ButtonKeyBindings, DataViewKeyBindings, DropdownKeyBindings, FocusKeyBindings, KeyBindings,
    KeySpec, TabsKeyBindings, ToggleKeyBindings,
};
pub use node::{
    AxisExpand, AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRepair, FocusRequest, FocusTarget, HintSource, HitRegion, LayoutAxis, LayoutCtx,
    LayoutOverflowDiagnostic, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint,
    LifecycleCtx, NonFocusable, OnBlur, OverflowPolicyName, Propagation, TreePath, TuiNode,
};
pub use preset::{BorderKind, DataViewPreset, DropdownPreset, Preset, TabsPreset, TabsVariant};
pub use runtime::{
    DispatchEffects, EventSource, FocusManager, FocusTransition, LayoutEngine, Renderer, Result,
    Scheduler, TerminalGuard, TreeApp, TreeDispatcher, run,
};
pub use scroll::{
    ScrollAxes, ScrollBehavior, ScrollDelta, ScrollGeometry, ScrollLayout, ScrollOffset,
    ScrollOutcome, ScrollPreset, ScrollSize, ScrollState, ScrollbarConfig, ScrollbarGutter,
    ScrollbarStyle, ScrollbarVisibility, line_width, paragraph_scroll, text_size,
};
pub use search::{
    MatchSpan, RankedSearchMatch, SearchMatch, SearchMode, search_match, search_ranked,
};
pub use separator::{GridSeparatorAxes, GridSeparators, Separator, SeparatorColorRole};
pub use spacing::{Gap, Padding};
pub use theme::{Theme, ThemeName};
pub use ui::{
    animation_settings, init, init_from_dir, keybindings, preset, set_keybindings, set_preset,
    set_theme, theme,
};

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
