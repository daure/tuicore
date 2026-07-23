use ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuicore::{ChildKey, EventRoute, Tab, Tabs, TabsVariant};

use crate::Msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalTabsExample {
    CenterMinimal,
    CenterUnderline,
    CenterBoxed,
    Top,
    Bottom,
    Left,
    Right,
    BottomSnackbar,
}

impl ModalTabsExample {
    pub(crate) fn variant(self) -> TabsVariant {
        match self {
            Self::CenterMinimal => TabsVariant::Minimal,
            Self::CenterUnderline => TabsVariant::Underline,
            Self::CenterBoxed
            | Self::Top
            | Self::Bottom
            | Self::Left
            | Self::Right
            | Self::BottomSnackbar => TabsVariant::Boxed,
        }
    }

    pub(crate) fn button_label(self) -> &'static str {
        match self {
            Self::CenterMinimal => "Open Style 1 tabs-as-dialog",
            Self::CenterUnderline => "Open Style 2 tabs-as-dialog",
            Self::CenterBoxed => "Open center boxed tabs-as-dialog",
            Self::Top => "Open top tabs dialog",
            Self::Bottom => "Open bottom tabs dialog",
            Self::Left => "Open left tabs dialog",
            Self::Right => "Open right tabs dialog",
            Self::BottomSnackbar => "Open 80% tabs snackbar",
        }
    }

    pub(crate) fn hotkey(self) -> &'static str {
        match self {
            Self::CenterMinimal => "td",
            Self::CenterUnderline => "tu",
            Self::CenterBoxed => "tc",
            Self::Top => "tt",
            Self::Bottom => "tb",
            Self::Left => "tl",
            Self::Right => "tr",
            Self::BottomSnackbar => "ts",
        }
    }
}

pub(crate) fn tabs_demo(variant: TabsVariant) -> Tabs<Msg> {
    let hotkeys = match variant {
        TabsVariant::Minimal => ["o", "u"],
        TabsVariant::Underline => ["v", "sa"],
        TabsVariant::Boxed => ["w", "e"],
    };
    Tabs::new(vec![
        Tab::text("Overview", "Simple tabs component for tuicore.").hotkey(hotkeys[0]),
        Tab::text("Usage", "Use Tab::new(title, node), then Tabs::new(tabs).").hotkey(hotkeys[1]),
    ])
    .variant(variant)
}

pub(crate) fn modal_tabs_dialog() -> Tabs<Msg> {
    Tabs::dialog(vec![
        Tab::text(
            "Overview",
            "This is the actual tabs-as-dialog demo. There is no Dialog wrapper, no extra title, and no nested border.",
        )
        .hotkey("o"),
        Tab::text(
            "Behavior",
            "The outer DialogLayer places this Tabs component, dims the gallery underneath, traps focus, and animates it in.",
        )
        .hotkey("b"),
        Tab::text(
            "Close",
            "Press x or Esc. The close affordance lives on the tab strip's top border line.",
        )
        .hotkey("c"),
    ])
    .on_close(Msg::ModalTabsClosed)
}

pub(crate) fn modal_tabs_preview_layout(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(15.min(area.height)), Constraint::Fill(1)])
        .areas(area)
}

pub(crate) fn modal_tabs_button_areas(area: Rect) -> [Rect; 8] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .spacing(1)
        .areas(area)
}

pub(crate) fn modal_tabs_open_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("modal-tabs-open-{index}"))
}

pub(crate) fn modal_tabs_open_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("modal-tabs-open-")?
        .parse()
        .ok()
        .filter(|index| *index < 8)
}

pub(crate) fn modal_tabs_open_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = modal_tabs_open_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

pub(crate) fn tab_demo_child_key(index: usize) -> ChildKey {
    ChildKey::new(format!("tab-demo-{index}"))
}

pub(crate) fn tab_demo_index(key: &ChildKey) -> Option<usize> {
    key.as_str()
        .strip_prefix("tab-demo-")?
        .parse()
        .ok()
        .filter(|index| *index < 4)
}

pub(crate) fn tab_demo_child_route(route: &EventRoute) -> Option<(usize, EventRoute)> {
    let first = route.path.first()?;
    let index = tab_demo_index(first)?;
    Some((index, EventRoute::new(route.path.without_first())))
}

pub(crate) fn tabs_areas(area: Rect) -> [Rect; 3] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .areas(area)
}

pub(crate) fn labeled_area(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Fill(1)])
        .areas(area)
}
