pub mod animation;
pub mod border;
pub mod children;
pub mod components;
mod config;
pub mod event;
pub mod focus;
pub mod hotkey;
pub mod keybindings;
pub mod node;
pub mod overlay;
pub mod preset;
pub mod runtime;
pub mod scroll;
pub mod search;
pub mod separator;
mod spacing;
pub mod store;
pub mod theme;
pub mod ui;

pub use animation::{
    Animated, AnimationSettings, AnimationSpec, ColorTween, Easing, ResolvedAnimationSpec,
    ScrollAnimator, TickResult, Tween, lerp_color,
};
pub use border::{BorderChars, border_chars, border_set};
pub use children::{ChildSlot, Children, DuplicateChildKey, MissingChildKey};
pub use components::{
    ActivationMode, AiDockKeyBindings, Button, ButtonOutcome, Calendar, CalendarEntryRole,
    CalendarOutcome, CalendarSpan, CalendarTypedEvent, CalendarView, CellContext, CheckState, Chip,
    ChipColorRole, Column, CrossAlign, CrossSize, DataView, DataViewEvent, DataViewFilter,
    DataViewOutcome, DataViewPagination, DataViewSort, DataViewTransformMode,
    DataViewTransformState, DataViewTypedEvent, DatePicker, DatePickerDropdown, DateTimeIndicator,
    DateTimeIndicatorFormat, DateTimePicker, DateTimePickerDropdown, DateTimePickerLayout, Dialog,
    DialogBackdrop, DialogCloseReason, DialogHost, DialogKeyBindings, DialogLayer,
    DialogLayerPlacement, DialogTitlePosition, DockChrome, DockSide, DockSpec, Dropdown,
    DropdownActionKeys, DropdownCommitMode, DropdownLabelPosition, DropdownOutcome,
    DropdownPopupDirection, DropdownSearchMode, DropdownVariant, Flex, FlexItem, Grid, GridItem,
    GridTrack, Header, InputChrome, InputOutcome, InputPanelChrome, List, ListOutcome, MainAlign,
    Menu, MenuActionKeys, MenuItem, MenuOutcome, MenuPopupDirection, MenuSearchMode,
    ModalCloseReason, Notification, NotificationCenter, NotificationId, NotificationKind, Overlay,
    OverlayAnchor, OverlaySize, Panel, PanelHost, PanelTitlePosition, Paragraph, ParagraphOverflow,
    PasswordInput, PickerOutcome, SelectedTag, SelectionGlyphs, SelectionMode,
    SelectionPropagation, SelectionTrigger, SortDirection, Spinner, Split, Stack, StackAlign,
    StackItem, StackSize, StatusBar, StatusBarKeyBindings, StatusBarMenuItem, StoreDebugView, Tab,
    Tabs, TabsSelectionMemory, TagInput, TagInputEvent, TextInput, TextInputKeyBindings,
    TextareaInput, TextareaInputKeyBindings, TimeField, TimePicker, TimePrecision, ToastIcons,
    ToastRack, Toggle, ToggleOutcome, ToggleStyle, TreeAdapter, TreeGlyphs, WeatherFetchError,
    WeatherForecastDay, WeatherForecastDialog, WeatherForecastError, WeatherIndicator,
    WeatherProviderConfig, WeatherReport, WeatherSummary, weather_condition_icon,
};
pub use components::{AiDock, LlmEvent, LlmEventKind, ToolPolicy};

pub use event::{
    ExternalEditorRequest, ExternalEditorResponse, HotkeyEvent, Key, KeyEvent, KeyModifiers,
    MouseButton, MouseEvent, MouseEventKind, TuiEvent, UnsupportedEvent,
};
pub use focus::{
    FocusChain, FocusDirection, FocusOutcome, FocusRouter, FocusRouterError, FocusWrap,
};
pub use hotkey::{
    HotkeyLabelMode, HotkeyMatch, HotkeySequenceMatcher, hotkey_badge_spans, hotkey_badge_width,
    hotkey_edge_spans, hotkey_label_spans, hotkey_sequence_to_event, hotkey_underline_style,
};
pub use keybindings::{
    ButtonKeyBindings, ClipboardKeyBindings, DataViewKeyBindings, DateTimePickerKeyBindings,
    DropdownKeyBindings, FocusKeyBindings, KeyBindings, KeyBindingsError, KeySpec,
    RuntimeKeyBindings, TabsKeyBindings, ToggleKeyBindings,
};
pub use node::{
    AxisExpand, AxisProposal, ChildKey, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusId,
    FocusRepair, FocusRequest, FocusTarget, HintSource, HitRegion, LayoutAxis, LayoutCtx,
    LayoutOverflowDiagnostic, LayoutProposal, LayoutResult, LayoutSize, LayoutSizeHint,
    LifecycleCtx, NonFocusable, OnBlur, OverflowPolicyName, Propagation, TreePath, TuiNode,
};
pub use overlay::{
    OutsideMousePolicy, OverlayId, OverlayLayer, OverlayLayoutEntry, OverlayManager, OverlayPolicy,
    OverlaySpec, RenderCtx,
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
pub use store::{
    DispatchOutcome, EventLog, InspectField, InspectValue, StateInspect, Store, StoreLike,
    StoreLogEntry, StoreLogPhase, StoreObserver,
};
pub use theme::{Theme, ThemeName};
pub use ui::{
    UiInitError, animation_settings, init, init_from_dir, keybindings, preset, set_keybindings,
    set_preset, set_theme, theme, try_init, try_init_from_dir,
};

#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
