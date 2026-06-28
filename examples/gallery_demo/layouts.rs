use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuicore::{
    Flex, FlexItem, Gap, Grid, GridItem, GridTrack, HintSource, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSize, LayoutSizeHint, Overlay, OverlayAnchor, OverlaySize, Separator,
    SeparatorColorRole, Split, Stack, StackAlign, StackItem, TuiNode,
};

use crate::Msg;

#[derive(Clone)]
pub(crate) struct DemoBox {
    title: &'static str,
    body: &'static str,
    size: LayoutSize,
}

impl DemoBox {
    pub(crate) fn new(title: &'static str, body: &'static str, width: u16, height: u16) -> Self {
        Self {
            title,
            body,
            size: LayoutSize::new(width, height),
        }
    }
}

impl TuiNode<Msg> for DemoBox {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(1, 1),
            preferred: self.size,
            expand: Default::default(),
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, area: Rect, _ctx: &mut tuicore::RenderCtx<'_>) {
        let title_style = Style::default()
            .fg(tuicore::theme().muted_fg())
            .add_modifier(Modifier::BOLD);
        let lines = vec![
            Line::from(Span::styled(self.title, title_style)),
            Line::from(self.body),
            Line::from(format!("rect: {}×{}", area.width, area.height)),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }
}

pub(crate) fn layout_flex_demo() -> Flex<Msg> {
    Flex::row()
        .padding(tuicore::Padding::horizontal_vertical(2, 1))
        .gap(2)
        .separator(Separator::new().role(SeparatorColorRole::Subtle))
        .child(
            "fixed",
            DemoBox::new("Fixed", "12 cols", 12, 3),
            FlexItem::fixed(12),
        )
        .child(
            "fit",
            DemoBox::new("FitContent", "measured child", 18, 3),
            FlexItem::fit_content(),
        )
        .child(
            "fill",
            DemoBox::new("Fill", "takes the rest", 12, 3),
            FlexItem::fill(1),
        )
}

pub(crate) fn layout_split_demo() -> Split<DemoBox, DemoBox> {
    Split::horizontal(
        DemoBox::new("Navigation", "ratio side pane", 20, 8),
        DemoBox::new("Workspace", "main region receives remainder", 40, 8),
    )
    .ratio(1, 2)
    .gap(1)
    .separator(Separator::new().role(SeparatorColorRole::Muted))
}

pub(crate) fn layout_stack_demo() -> Stack<Msg> {
    Stack::new()
        .child(
            "base",
            DemoBox::new("Base layer", "fills all available space", 30, 8),
            StackItem::new(),
        )
        .child(
            "center",
            DemoBox::new("Centered empty state", "fit-content layer", 26, 4),
            StackItem::new()
                .fit_content()
                .align(StackAlign::Center, StackAlign::Center),
        )
        .child(
            "badge",
            DemoBox::new("Badge", "top right", 18, 3),
            StackItem::new()
                .fixed(18, 3)
                .align(StackAlign::End, StackAlign::Start)
                .inset(tuicore::Padding::all(1)),
        )
}

pub(crate) fn layout_layered_demo() -> Overlay<DemoBox, DemoBox> {
    Overlay::new(
        DemoBox::new(
            "Base content",
            "normal flow size comes from this child",
            32,
            8,
        ),
        DemoBox::new("Popover", "anchored overlay", 24, 5),
    )
    .anchor(OverlayAnchor::BottomRight)
    .layer_size(OverlaySize::FitContent)
}

pub(crate) fn layout_grid_demo() -> Grid<Msg> {
    Grid::new()
        .columns([
            GridTrack::fixed(14),
            GridTrack::fit_content(),
            GridTrack::fill(1),
        ])
        .rows([
            GridTrack::fixed(4),
            GridTrack::percent(35),
            GridTrack::fill(1),
        ])
        .gaps(Gap::new(1, 2))
        .separator(Separator::new().role(SeparatorColorRole::Muted))
        .padding(tuicore::Padding::all(1))
        .child(
            "filters",
            DemoBox::new("Filters", "fixed track", 10, 3),
            GridItem::new(0, 0),
        )
        .child(
            "summary",
            DemoBox::new("Summary", "fit-content track", 18, 3),
            GridItem::new(0, 1),
        )
        .child(
            "chart",
            DemoBox::new("Chart", "fills remaining width", 28, 8),
            GridItem::new(0, 2).span(2, 1),
        )
        .child(
            "table",
            DemoBox::new("Table", "spans first two columns", 30, 8),
            GridItem::new(1, 0).span(2, 2),
        )
}

pub(crate) fn render_layout_intro(frame: &mut Frame, area: Rect, text: &'static str) {
    frame.render_widget(Paragraph::new(text), layout_demo_header(area));
}

fn layout_demo_header(area: Rect) -> Rect {
    layout_demo_areas(area)[0]
}

pub(crate) fn layout_demo_body(area: Rect) -> Rect {
    layout_demo_areas(area)[1]
}

fn layout_demo_areas(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .areas(area)
}
