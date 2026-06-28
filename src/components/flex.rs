use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Direction, Rect};

use crate::spacing::Padding;
use crate::{
    AnimationSettings, AxisProposal, ChildKey, Children, DuplicateChildKey, EventCtx, EventOutcome,
    EventRoute, FocusCtx, FocusTarget, HintSource, LayoutAxis, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSize, LayoutSizeHint, LifecycleCtx, MissingChildKey, OverflowPolicyName,
    TickResult, TuiEvent, TuiNode,
};
use crate::{Separator, separator};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainAlign {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAlign {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossSize {
    Auto,
    Fixed(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlexItem {
    main: FlexMain,
    cross: CrossSize,
    align_self: Option<CrossAlign>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlexMain {
    Fixed(u16),
    Fill(u16),
    Percent(u16),
    FitContent,
}

pub struct Flex<M = ()> {
    direction: Direction,
    children: Children<M>,
    items: Vec<FlexChild>,
    rects: Vec<(ChildKey, Rect)>,
    separator_rects: Vec<Rect>,
    gap: u16,
    padding: Padding,
    justify: MainAlign,
    align: CrossAlign,
    separator: Option<Separator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlexChild {
    key: ChildKey,
    item: FlexItem,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self::fit_content()
    }
}

impl FlexItem {
    pub fn fixed(size: u16) -> Self {
        Self {
            main: FlexMain::Fixed(size),
            cross: CrossSize::Auto,
            align_self: None,
        }
    }

    pub fn fill(weight: u16) -> Self {
        Self {
            main: FlexMain::Fill(weight.max(1)),
            cross: CrossSize::Auto,
            align_self: None,
        }
    }

    pub fn percent(percent: u16) -> Self {
        Self {
            main: FlexMain::Percent(percent.min(100)),
            cross: CrossSize::Auto,
            align_self: None,
        }
    }

    pub fn fit_content() -> Self {
        Self {
            main: FlexMain::FitContent,
            cross: CrossSize::Auto,
            align_self: None,
        }
    }

    pub fn content() -> Self {
        Self::fit_content()
    }

    pub fn cross_size(mut self, cross: CrossSize) -> Self {
        self.cross = cross;
        self
    }

    pub fn align_self(mut self, align: CrossAlign) -> Self {
        self.align_self = Some(align);
        self
    }
}

impl<M> Flex<M> {
    pub fn row() -> Self {
        Self::new(Direction::Horizontal)
    }

    pub fn column() -> Self {
        Self::new(Direction::Vertical)
    }

    pub fn gap(mut self, gap: u16) -> Self {
        self.gap = gap;
        self
    }

    pub fn separator(mut self, separator: Separator) -> Self {
        self.separator = Some(separator);
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn justify(mut self, justify: MainAlign) -> Self {
        self.justify = justify;
        self
    }

    pub fn align(mut self, align: CrossAlign) -> Self {
        self.align = align;
        self
    }

    pub fn children(&self) -> &Children<M> {
        &self.children
    }

    pub fn child_rect(&self, key: &ChildKey) -> Option<Rect> {
        self.rects
            .iter()
            .find_map(|(child_key, rect)| (child_key == key).then_some(*rect))
    }

    fn new(direction: Direction) -> Self {
        Self {
            direction,
            children: Children::new(),
            items: Vec::new(),
            rects: Vec::new(),
            separator_rects: Vec::new(),
            gap: 0,
            padding: Padding::default(),
            justify: MainAlign::Start,
            align: CrossAlign::Stretch,
            separator: None,
        }
    }
}

impl<M> Flex<M>
where
    M: 'static,
{
    pub fn child<C>(mut self, key: impl Into<ChildKey>, child: C, item: FlexItem) -> Self
    where
        C: TuiNode<M> + 'static,
    {
        if let Err(error) = self.try_push(key, child, item) {
            panic!("duplicate child key: {}", error.key.as_str());
        }
        self
    }

    pub fn try_child<C>(
        mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
    ) -> Result<Self, DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        self.try_push(key, child, item)?;
        Ok(self)
    }

    fn try_push<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children = std::mem::take(&mut self.children).try_child(key.clone(), child)?;
        self.items.push(FlexChild { key, item });
        Ok(())
    }

    pub fn insert<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
        ctx: &mut EventCtx<M>,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children.insert(key.clone(), child, ctx)?;
        self.items.push(FlexChild { key, item });
        Ok(())
    }

    pub fn replace<C>(
        &mut self,
        key: impl Into<ChildKey>,
        child: C,
        item: FlexItem,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        let old = self.children.replace(key.clone(), child, ctx)?;
        if let Some(flex_child) = self.items.iter_mut().find(|child| child.key == key) {
            flex_child.item = item;
        }
        Ok(old)
    }

    pub fn remove(
        &mut self,
        key: impl Into<ChildKey>,
        ctx: &mut EventCtx<M>,
    ) -> Result<Box<dyn TuiNode<M>>, MissingChildKey> {
        let key = key.into();
        let old = self.children.remove(key.clone(), ctx)?;
        self.items.retain(|child| child.key != key);
        Ok(old)
    }
}

impl<M> TuiNode<M> for Flex<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let gap = self.total_spacing(self.items.len());
        let padding_width = self.padding.left.saturating_add(self.padding.right);
        let padding_height = self.padding.top.saturating_add(self.padding.bottom);
        let (main_padding, cross_padding, main_proposal, cross_proposal) = match self.direction {
            Direction::Horizontal => (
                padding_width,
                padding_height,
                proposal.width,
                proposal.height,
            ),
            Direction::Vertical => (
                padding_height,
                padding_width,
                proposal.height,
                proposal.width,
            ),
        };
        let main_available = inner_axis_bound(main_proposal, main_padding);
        let cross_available = inner_axis_bound(cross_proposal, cross_padding);
        let main_without_spacing =
            main_available.map(|available| u32::from(available).saturating_sub(u32::from(gap)));
        let child_proposal = self.measure_child_proposal(main_without_spacing, cross_available);
        let (min_main, preferred_main) =
            self.measure_main_lengths(child_proposal, main_without_spacing);
        let (min_cross, preferred_cross) = self.measure_cross_lengths(child_proposal);
        let (min, preferred) = match self.direction {
            Direction::Horizontal => (
                LayoutSize::new(min_main.saturating_add(gap), min_cross),
                LayoutSize::new(preferred_main.saturating_add(gap), preferred_cross),
            ),
            Direction::Vertical => (
                LayoutSize::new(min_cross, min_main.saturating_add(gap)),
                LayoutSize::new(preferred_cross, preferred_main.saturating_add(gap)),
            ),
        };

        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(
                min.width.saturating_add(padding_width),
                min.height.saturating_add(padding_height),
            ),
            preferred: LayoutSize::new(
                preferred.width.saturating_add(padding_width),
                preferred.height.saturating_add(padding_height),
            ),
            expand: crate::AxisExpand {
                width: matches!(self.direction, Direction::Horizontal)
                    && self
                        .items
                        .iter()
                        .any(|child| matches!(child.item.main, FlexMain::Fill(_))),
                height: matches!(self.direction, Direction::Vertical)
                    && self
                        .items
                        .iter()
                        .any(|child| matches!(child.item.main, FlexMain::Fill(_))),
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let (rects, separator_rects, overflow) = self.calculate_layout(area);
        self.rects = rects;
        self.separator_rects = separator_rects;
        if let Some((needed, available)) = overflow {
            let axis = match self.direction {
                Direction::Horizontal => LayoutAxis::Width,
                Direction::Vertical => LayoutAxis::Height,
            };
            ctx.record_overflow(axis, needed, available, OverflowPolicyName::Clip);
        }
        for (key, rect) in &self.rects {
            self.children.layout_child(key, *rect, ctx);
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut crate::RenderCtx<'a>) {
        for (key, rect) in &self.rects {
            if let Some(child) = self.children.get(key) {
                child.render(frame, *rect, ctx);
            }
        }
        if let Some(separator) = self.separator {
            for rect in &self.separator_rects {
                match self.direction {
                    Direction::Horizontal => separator::draw_vertical(frame, *rect, separator),
                    Direction::Vertical => separator::draw_horizontal(frame, *rect, separator),
                }
            }
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        let child = self.children.dispatch_routed_child(route, event, ctx);
        child.bubble(ctx, |ctx| self.event(event, ctx))
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.children.tick(dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.children.dispatch_focus_target(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.children.destroy(ctx);
    }
}

impl<M> Flex<M> {
    fn calculate_layout(
        &self,
        area: Rect,
    ) -> (Vec<(ChildKey, Rect)>, Vec<Rect>, Option<(u16, u16)>) {
        let inner = self.inner_area(area);
        let main_available = self.main_len(inner);
        let count = self.items.len();
        if count == 0 {
            return (Vec::new(), Vec::new(), None);
        }

        let gap_count = u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX);
        let separator_cell = u32::from(separator::separator_cell(
            self.separator.is_some(),
            count.saturating_sub(1),
        ));
        let base_between = u32::from(self.gap).saturating_add(separator_cell);
        let base_spacing = base_between.saturating_mul(gap_count);
        let main_without_spacing = u32::from(main_available).saturating_sub(base_spacing);
        let lengths = self.main_lengths(inner, main_without_spacing);
        let used = lengths
            .iter()
            .fold(0u32, |used, length| used.saturating_add(u32::from(*length)));
        let overflow = (used.saturating_add(base_spacing) > u32::from(main_available)).then(|| {
            (
                clamp_u32_to_u16(used.saturating_add(base_spacing)),
                main_available,
            )
        });
        let spare = main_without_spacing.saturating_sub(used);
        let JustifySpaces { leading, between } =
            justify_spaces(self.justify, count, clamp_u32_to_u16(base_between), spare);

        let mut cursor = leading;
        let mut separator_rects = Vec::new();
        let rects = self
            .items
            .iter()
            .zip(lengths)
            .enumerate()
            .map(|(index, (child, main))| {
                let rect = self.rect_for(inner, cursor, main, child);
                if separator_cell > 0
                    && let Some(between) = between.get(index)
                    && let Some(rect) =
                        self.separator_rect(inner, cursor.saturating_add(u32::from(main)), *between)
                {
                    separator_rects.push(rect);
                }
                cursor = cursor
                    .saturating_add(u32::from(main))
                    .saturating_add(between.get(index).copied().unwrap_or(0));
                (child.key.clone(), rect)
            })
            .collect::<Vec<_>>();
        (rects, separator_rects, overflow)
    }

    fn total_spacing(&self, count: usize) -> u16 {
        let gaps = count.saturating_sub(1).min(usize::from(u16::MAX)) as u16;
        self.gap
            .saturating_mul(gaps)
            .saturating_add(separator::separator_slots(self.separator.is_some(), count))
    }

    fn inner_area(&self, area: Rect) -> Rect {
        let x = area.x.saturating_add(self.padding.left);
        let y = area.y.saturating_add(self.padding.top);
        let width = area
            .width
            .saturating_sub(self.padding.left.saturating_add(self.padding.right));
        let height = area
            .height
            .saturating_sub(self.padding.top.saturating_add(self.padding.bottom));
        Rect::new(x, y, width, height)
    }

    fn main_lengths(&self, area: Rect, available: u32) -> Vec<u16> {
        let proposal = self.child_measure_proposal(area, clamp_u32_to_u16(available));
        let mut bases = Vec::with_capacity(self.items.len());
        let mut minimums = Vec::with_capacity(self.items.len());
        for child in &self.items {
            let (basis, minimum) = match child.item.main {
                FlexMain::Fixed(value) => (u32::from(value), u32::from(value)),
                FlexMain::Percent(percent) => {
                    let basis = available.saturating_mul(u32::from(percent)) / 100;
                    (basis, basis)
                }
                FlexMain::Fill(_) => (0, 0),
                FlexMain::FitContent => self.fit_content_basis(child, proposal, available),
            };
            bases.push(basis);
            minimums.push(minimum);
        }

        shrink_fit_content(&self.items, &mut bases, &minimums, available);

        let reserved = bases
            .iter()
            .fold(0u32, |sum, length| sum.saturating_add(*length));
        let fill_space = available.saturating_sub(reserved);
        let fill_weight = self
            .items
            .iter()
            .map(|child| match child.item.main {
                FlexMain::Fill(weight) => u32::from(weight),
                _ => 0,
            })
            .fold(0u32, |sum, weight| sum.saturating_add(weight));

        let mut lengths = bases;
        let mut distributed = 0u32;
        for (index, child) in self.items.iter().enumerate() {
            if let FlexMain::Fill(weight) = child.item.main
                && fill_weight > 0
            {
                let share = fill_space.saturating_mul(u32::from(weight)) / fill_weight;
                lengths[index] = share;
                distributed = distributed.saturating_add(share);
            }
        }

        let mut remainder = fill_space.saturating_sub(distributed);
        for (index, child) in self.items.iter().enumerate() {
            if remainder == 0 {
                break;
            }
            if matches!(child.item.main, FlexMain::Fill(_)) {
                lengths[index] = lengths[index].saturating_add(1);
                remainder -= 1;
            }
        }

        lengths.into_iter().map(clamp_u32_to_u16).collect()
    }

    fn rect_for(&self, area: Rect, main_offset: u32, main: u16, child: &FlexChild) -> Rect {
        let cross_available = self.cross_len(area);
        let align = child.item.align_self.unwrap_or(self.align);
        let cross_len = match (child.item.cross, align) {
            (CrossSize::Fixed(size), _) => size.min(cross_available),
            (CrossSize::Auto, CrossAlign::Stretch) => cross_available,
            (CrossSize::Auto, _) => self
                .children
                .measure_child(&child.key, self.child_measure_proposal(area, main))
                .filter(|hint| hint.source == HintSource::Measured)
                .map(|hint| self.cross_hint(hint.preferred).min(cross_available))
                .unwrap_or(cross_available),
        };
        let cross_offset = match align {
            CrossAlign::Start | CrossAlign::Stretch => 0,
            CrossAlign::Center => cross_available.saturating_sub(cross_len) / 2,
            CrossAlign::End => cross_available.saturating_sub(cross_len),
        };
        let main_remaining = self
            .main_len(area)
            .saturating_sub(clamp_u32_to_u16(main_offset));
        let main_offset = clamp_u32_to_u16(main_offset);
        let main_len = main.min(main_remaining);

        match self.direction {
            Direction::Horizontal => Rect::new(
                area.x.saturating_add(main_offset),
                area.y.saturating_add(cross_offset),
                main_len,
                cross_len,
            ),
            Direction::Vertical => Rect::new(
                area.x.saturating_add(cross_offset),
                area.y.saturating_add(main_offset),
                cross_len,
                main_len,
            ),
        }
    }

    fn separator_rect(&self, area: Rect, slot_start: u32, between: u32) -> Option<Rect> {
        if between == 0 {
            return None;
        }
        let main_offset = slot_start.saturating_add(between / 2);
        if main_offset >= u32::from(self.main_len(area)) {
            return None;
        }
        let main_offset = clamp_u32_to_u16(main_offset);
        Some(match self.direction {
            Direction::Horizontal => {
                Rect::new(area.x.saturating_add(main_offset), area.y, 1, area.height)
            }
            Direction::Vertical => {
                Rect::new(area.x, area.y.saturating_add(main_offset), area.width, 1)
            }
        })
    }

    fn main_len(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        }
    }

    fn cross_len(&self, area: Rect) -> u16 {
        match self.direction {
            Direction::Horizontal => area.height,
            Direction::Vertical => area.width,
        }
    }

    fn child_measure_proposal(&self, area: Rect, main: u16) -> LayoutProposal {
        match self.direction {
            Direction::Horizontal => LayoutProposal {
                width: crate::AxisProposal::AtMost(main),
                height: crate::AxisProposal::AtMost(area.height),
            },
            Direction::Vertical => LayoutProposal {
                width: crate::AxisProposal::AtMost(area.width),
                height: crate::AxisProposal::AtMost(main),
            },
        }
    }

    fn measure_child_proposal(&self, main: Option<u32>, cross: Option<u16>) -> LayoutProposal {
        let main = main
            .map(clamp_u32_to_u16)
            .map(AxisProposal::AtMost)
            .unwrap_or(AxisProposal::Unbounded);
        let cross = cross
            .map(AxisProposal::AtMost)
            .unwrap_or(AxisProposal::Unbounded);
        match self.direction {
            Direction::Horizontal => LayoutProposal {
                width: main,
                height: cross,
            },
            Direction::Vertical => LayoutProposal {
                width: cross,
                height: main,
            },
        }
    }

    fn measure_main_lengths(&self, proposal: LayoutProposal, available: Option<u32>) -> (u16, u16) {
        let mut min_total = 0u32;
        let mut bases = Vec::with_capacity(self.items.len());
        let mut minimums = Vec::with_capacity(self.items.len());
        for child in &self.items {
            let (min, preferred) = match child.item.main {
                FlexMain::Fixed(value) => (u32::from(value), u32::from(value)),
                FlexMain::Percent(percent) => {
                    let basis = available
                        .map(|available| available.saturating_mul(u32::from(percent)) / 100)
                        .unwrap_or_else(|| {
                            self.children
                                .measure_child(&child.key, proposal)
                                .filter(|hint| hint.source == HintSource::Measured)
                                .map(|hint| u32::from(self.main_hint(hint.preferred)))
                                .unwrap_or(0)
                        });
                    (basis, basis)
                }
                FlexMain::Fill(_) => (0, 0),
                FlexMain::FitContent => {
                    let (preferred, min) =
                        self.fit_content_basis(child, proposal, available.unwrap_or(1));
                    (min, preferred)
                }
            };
            min_total = min_total.saturating_add(min);
            bases.push(preferred);
            minimums.push(min);
        }

        if let Some(available) = available {
            shrink_fit_content(&self.items, &mut bases, &minimums, available);
            self.distribute_fill_lengths(&mut bases, available);
        }

        let preferred_total = bases
            .into_iter()
            .fold(0u32, |sum, length| sum.saturating_add(length));
        (
            clamp_u32_to_u16(min_total),
            clamp_u32_to_u16(preferred_total),
        )
    }

    fn distribute_fill_lengths(&self, lengths: &mut [u32], available: u32) {
        let reserved = lengths
            .iter()
            .fold(0u32, |sum, length| sum.saturating_add(*length));
        let fill_space = available.saturating_sub(reserved);
        let fill_weight = self
            .items
            .iter()
            .map(|child| match child.item.main {
                FlexMain::Fill(weight) => u32::from(weight),
                _ => 0,
            })
            .fold(0u32, |sum, weight| sum.saturating_add(weight));
        let mut distributed = 0u32;
        for (index, child) in self.items.iter().enumerate() {
            if let FlexMain::Fill(weight) = child.item.main
                && fill_weight > 0
            {
                let share = fill_space.saturating_mul(u32::from(weight)) / fill_weight;
                lengths[index] = share;
                distributed = distributed.saturating_add(share);
            }
        }
        let mut remainder = fill_space.saturating_sub(distributed);
        for (index, child) in self.items.iter().enumerate() {
            if remainder == 0 {
                break;
            }
            if matches!(child.item.main, FlexMain::Fill(_)) {
                lengths[index] = lengths[index].saturating_add(1);
                remainder -= 1;
            }
        }
    }

    fn measure_cross_lengths(&self, proposal: LayoutProposal) -> (u16, u16) {
        let mut min = 0;
        let mut preferred = 0;
        for child in &self.items {
            match child.item.cross {
                CrossSize::Fixed(size) => {
                    min = min.max(size);
                    preferred = preferred.max(size);
                }
                CrossSize::Auto => {
                    if let Some(hint) = self.children.measure_child(&child.key, proposal) {
                        min = min.max(self.cross_hint(hint.min));
                        preferred = preferred.max(self.cross_hint(hint.preferred));
                    }
                }
            }
        }
        (min, preferred)
    }

    fn fit_content_basis(
        &self,
        child: &FlexChild,
        proposal: LayoutProposal,
        available: u32,
    ) -> (u32, u32) {
        let Some(hint) = self.children.measure_child(&child.key, proposal) else {
            return (0, 0);
        };
        if hint.source == HintSource::LegacyUnmeasured {
            return ((available > 0) as u32, 0);
        }
        (
            u32::from(self.main_hint(hint.preferred)),
            u32::from(self.main_hint(hint.min)),
        )
    }

    fn main_hint(&self, size: LayoutSize) -> u16 {
        match self.direction {
            Direction::Horizontal => size.width,
            Direction::Vertical => size.height,
        }
    }

    fn cross_hint(&self, size: LayoutSize) -> u16 {
        match self.direction {
            Direction::Horizontal => size.height,
            Direction::Vertical => size.width,
        }
    }
}

struct JustifySpaces {
    leading: u32,
    between: Vec<u32>,
}

fn justify_spaces(justify: MainAlign, count: usize, gap: u16, spare: u32) -> JustifySpaces {
    let base_gap = u32::from(gap);
    let mut between = vec![base_gap; count.saturating_sub(1)];
    let leading = match justify {
        MainAlign::Start => 0,
        MainAlign::Center => spare / 2,
        MainAlign::End => spare,
        MainAlign::SpaceBetween => {
            if count > 1 {
                add_distributed_spare(&mut between, spare);
            }
            0
        }
        MainAlign::SpaceAround => {
            let halves = distribute_spare(spare, count.saturating_mul(2));
            for (index, space) in between.iter_mut().enumerate() {
                *space = space
                    .saturating_add(halves[index.saturating_mul(2).saturating_add(1)])
                    .saturating_add(halves[index.saturating_mul(2).saturating_add(2)]);
            }
            halves.first().copied().unwrap_or(0)
        }
        MainAlign::SpaceEvenly => {
            let spaces = distribute_spare(spare, count.saturating_add(1));
            for (index, space) in between.iter_mut().enumerate() {
                *space = space.saturating_add(spaces[index.saturating_add(1)]);
            }
            spaces.first().copied().unwrap_or(0)
        }
    };
    JustifySpaces { leading, between }
}

fn add_distributed_spare(spaces: &mut [u32], spare: u32) {
    let additions = distribute_spare(spare, spaces.len());
    for (space, addition) in spaces.iter_mut().zip(additions) {
        *space = space.saturating_add(addition);
    }
}

fn distribute_spare(spare: u32, buckets: usize) -> Vec<u32> {
    if buckets == 0 {
        return Vec::new();
    }
    let bucket_count = u32::try_from(buckets).unwrap_or(u32::MAX);
    let base = spare / bucket_count;
    let mut remainder = spare % bucket_count;
    (0..buckets)
        .map(|_| {
            let value = base.saturating_add((remainder > 0) as u32);
            remainder = remainder.saturating_sub(1);
            value
        })
        .collect()
}

fn shrink_fit_content(items: &[FlexChild], bases: &mut [u32], minimums: &[u32], available: u32) {
    let total = bases
        .iter()
        .fold(0u32, |sum, basis| sum.saturating_add(*basis));
    let mut debt = total.saturating_sub(available);
    if debt == 0 {
        return;
    }

    loop {
        let mut shrunk_any = false;
        for (index, child) in items.iter().enumerate() {
            if debt == 0 {
                return;
            }
            if !matches!(child.item.main, FlexMain::FitContent) {
                continue;
            }
            if bases[index] > minimums[index] {
                bases[index] -= 1;
                debt -= 1;
                shrunk_any = true;
            }
        }
        if !shrunk_any {
            break;
        }
    }
}

fn clamp_u32_to_u16(value: u32) -> u16 {
    value.min(u32::from(u16::MAX)) as u16
}

fn inner_axis_bound(proposal: AxisProposal, padding: u16) -> Option<u16> {
    match proposal {
        AxisProposal::AtMost(value) | AxisProposal::Exact(value) => {
            Some(value.saturating_sub(padding))
        }
        AxisProposal::Unbounded => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use super::*;
    use crate::{FocusId, Key, KeyEvent, Propagation, TreePath};

    #[derive(Default)]
    struct Probe {
        ticks: Rc<RefCell<usize>>,
    }

    impl TuiNode<()> for Probe {
        fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
            ctx.register_focusable(FocusId::new("probe"), area, true);
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut crate::RenderCtx<'_>) {}

        fn event(&mut self, _event: &TuiEvent, ctx: &mut EventCtx<()>) -> EventOutcome {
            ctx.stop_propagation();
            EventOutcome::Handled
        }

        fn tick(&mut self, _dt: Duration, _settings: AnimationSettings) -> TickResult {
            *self.ticks.borrow_mut() += 1;
            TickResult::IDLE
        }
    }

    struct MeasuredProbe {
        min: LayoutSize,
        preferred: LayoutSize,
    }

    impl TuiNode<()> for MeasuredProbe {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint {
                source: HintSource::Measured,
                min: self.min,
                preferred: self.preferred,
                expand: crate::AxisExpand::default(),
            }
            .normalized(proposal)
        }

        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut crate::RenderCtx<'_>) {}
    }

    #[test]
    fn flex_row_calculates_fixed_percent_fill_gap_and_padding() {
        let mut flex = Flex::row()
            .padding(Padding::horizontal_vertical(2, 1))
            .gap(1)
            .child("fixed", Probe::default(), FlexItem::fixed(10))
            .child("percent", Probe::default(), FlexItem::percent(50))
            .child("fill", Probe::default(), FlexItem::fill(1));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 100, 5), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("fixed")).unwrap(),
            Rect::new(2, 1, 10, 3)
        );
        assert_eq!(
            flex.child_rect(&ChildKey::from("percent")).unwrap().width,
            47
        );
        assert_eq!(flex.child_rect(&ChildKey::from("fill")).unwrap().width, 37);
        assert_eq!(
            ctx.focus_targets()[0].path,
            TreePath::from_keys([ChildKey::from("fixed")])
        );
    }

    #[test]
    fn flex_row_separator_reserves_space_between_items() {
        let mut flex = Flex::row()
            .gap(2)
            .separator(Separator::new())
            .child("one", Probe::default(), FlexItem::fixed(2))
            .child("two", Probe::default(), FlexItem::fixed(2));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 7, 3), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().x, 0);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 5);
        assert_eq!(flex.separator_rects, vec![Rect::new(3, 0, 1, 3)]);
    }

    #[test]
    fn flex_column_separator_reserves_space_between_items() {
        let mut flex = Flex::column()
            .separator(Separator::new())
            .child("one", Probe::default(), FlexItem::fixed(2))
            .child("two", Probe::default(), FlexItem::fixed(2));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 4, 5), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().y, 0);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().y, 3);
        assert_eq!(flex.separator_rects, vec![Rect::new(0, 2, 4, 1)]);
    }

    #[test]
    fn flex_separator_omits_zero_or_one_child() {
        let mut empty = Flex::<()>::row().separator(Separator::new());
        let mut one = Flex::row().separator(Separator::new()).child(
            "one",
            Probe::default(),
            FlexItem::fixed(2),
        );
        let mut ctx = LayoutCtx::new();

        empty.layout(Rect::new(0, 0, 4, 1), &mut ctx);
        one.layout(Rect::new(0, 0, 4, 1), &mut ctx);

        assert!(empty.separator_rects.is_empty());
        assert!(one.separator_rects.is_empty());
    }

    #[test]
    fn flex_measure_includes_separator_space() {
        let flex = Flex::row()
            .separator(Separator::new())
            .child(
                "one",
                MeasuredProbe {
                    min: LayoutSize::new(2, 1),
                    preferred: LayoutSize::new(3, 1),
                },
                FlexItem::fit_content(),
            )
            .child(
                "two",
                MeasuredProbe {
                    min: LayoutSize::new(2, 1),
                    preferred: LayoutSize::new(3, 1),
                },
                FlexItem::fit_content(),
            );

        let hint = flex.measure(LayoutProposal::unbounded());

        assert_eq!(hint.min.width, 5);
        assert_eq!(hint.preferred.width, 7);
    }

    #[test]
    fn flex_measure_applies_fixed_percent_fill_and_padding() {
        let flex = Flex::row()
            .padding(Padding::horizontal_vertical(2, 1))
            .gap(1)
            .child(
                "fixed",
                MeasuredProbe {
                    min: LayoutSize::new(1, 1),
                    preferred: LayoutSize::new(1, 1),
                },
                FlexItem::fixed(10),
            )
            .child(
                "percent",
                MeasuredProbe {
                    min: LayoutSize::new(1, 1),
                    preferred: LayoutSize::new(1, 1),
                },
                FlexItem::percent(50),
            )
            .child(
                "fill",
                MeasuredProbe {
                    min: LayoutSize::new(1, 1),
                    preferred: LayoutSize::new(1, 1),
                },
                FlexItem::fill(1),
            );

        let hint = flex.measure(LayoutProposal::at_most(100, 5));

        assert_eq!(hint.preferred, LayoutSize::new(100, 3));
    }

    #[test]
    fn flex_measure_uses_fit_content_and_legacy_fallback() {
        let flex = Flex::row()
            .child(
                "content",
                MeasuredProbe {
                    min: LayoutSize::new(2, 1),
                    preferred: LayoutSize::new(8, 1),
                },
                FlexItem::fit_content(),
            )
            .child("legacy", Probe::default(), FlexItem::fit_content());

        let hint = flex.measure(LayoutProposal::unbounded());

        assert_eq!(hint.min.width, 2);
        assert_eq!(hint.preferred.width, 9);
    }

    #[test]
    fn flex_space_between_distributes_non_divisible_spare_left_to_right() {
        let mut flex = Flex::row()
            .gap(1)
            .justify(MainAlign::SpaceBetween)
            .child("one", Probe::default(), FlexItem::fixed(2))
            .child("two", Probe::default(), FlexItem::fixed(2))
            .child("three", Probe::default(), FlexItem::fixed(2));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 19, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().x, 0);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 9);
        assert_eq!(flex.child_rect(&ChildKey::from("three")).unwrap().x, 17);
    }

    #[test]
    fn flex_space_around_distributes_half_spaces_left_to_right() {
        let mut flex = Flex::row()
            .gap(1)
            .justify(MainAlign::SpaceAround)
            .child("one", Probe::default(), FlexItem::fixed(2))
            .child("two", Probe::default(), FlexItem::fixed(2))
            .child("three", Probe::default(), FlexItem::fixed(2));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 21, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().x, 3);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 10);
        assert_eq!(flex.child_rect(&ChildKey::from("three")).unwrap().x, 17);
    }

    #[test]
    fn flex_space_evenly_distributes_spaces_left_to_right() {
        let mut flex = Flex::row()
            .gap(1)
            .justify(MainAlign::SpaceEvenly)
            .child("one", Probe::default(), FlexItem::fixed(2))
            .child("two", Probe::default(), FlexItem::fixed(2))
            .child("three", Probe::default(), FlexItem::fixed(2));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 21, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().x, 4);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 10);
        assert_eq!(flex.child_rect(&ChildKey::from("three")).unwrap().x, 16);
    }

    #[test]
    fn flex_align_self_overrides_container_cross_alignment() {
        let mut flex = Flex::row()
            .align(CrossAlign::End)
            .child(
                "override",
                Probe::default(),
                FlexItem::fixed(2)
                    .cross_size(CrossSize::Fixed(2))
                    .align_self(CrossAlign::Start),
            )
            .child(
                "container",
                Probe::default(),
                FlexItem::fixed(2).cross_size(CrossSize::Fixed(2)),
            );
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 4, 10), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("override")).unwrap().y, 0);
        assert_eq!(flex.child_rect(&ChildKey::from("container")).unwrap().y, 8);
    }

    #[test]
    fn flex_ticks_each_child_once() {
        let ticks = Rc::new(RefCell::new(0));
        let mut flex = Flex::column()
            .child(
                "one",
                Probe {
                    ticks: Rc::clone(&ticks),
                },
                FlexItem::fixed(1),
            )
            .child(
                "two",
                Probe {
                    ticks: Rc::clone(&ticks),
                },
                FlexItem::fill(1),
            );

        flex.tick(Duration::from_millis(16), AnimationSettings::default());

        assert_eq!(*ticks.borrow(), 2);
    }

    #[test]
    fn flex_routes_child_events_and_preserves_stop_propagation() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fill(1));
        let route = EventRoute::new(TreePath::from_keys([ChildKey::from("one")]));
        let mut ctx = EventCtx::default();

        let outcome =
            flex.dispatch_event(&route, &TuiEvent::Key(KeyEvent::from(Key::Enter)), &mut ctx);

        assert_eq!(outcome, EventOutcome::Handled);
        assert_eq!(ctx.propagation(), Propagation::Stopped);
    }

    #[test]
    fn flex_insert_updates_layout_items_and_children_together() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fixed(10));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        flex.insert("two", Probe::default(), FlexItem::fixed(20), &mut ctx)
            .unwrap();
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(flex.children().contains_key(&ChildKey::from("two")));
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().width, 20);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_replace_updates_layout_item_for_existing_child() {
        let mut flex = Flex::row().child("one", Probe::default(), FlexItem::fixed(10));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        let old = flex
            .replace("one", Probe::default(), FlexItem::fixed(25), &mut ctx)
            .unwrap();
        drop(old);
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(flex.children().contains_key(&ChildKey::from("one")));
        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().width, 25);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_remove_updates_layout_items_and_children_together() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fixed(10))
            .child("two", Probe::default(), FlexItem::fixed(20));
        let mut ctx = EventCtx::default();
        let mut layout = LayoutCtx::new();

        let old = flex.remove("one", &mut ctx).unwrap();
        drop(old);
        flex.layout(Rect::new(0, 0, 40, 1), &mut layout);

        assert!(!flex.children().contains_key(&ChildKey::from("one")));
        assert!(flex.child_rect(&ChildKey::from("one")).is_none());
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 0);
        assert!(ctx.layout_requested());
    }

    #[test]
    fn flex_large_fixed_lengths_do_not_overflow() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fixed(50_000))
            .child("two", Probe::default(), FlexItem::fixed(50_000));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, u16::MAX, 1), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("one")).unwrap().width,
            50_000
        );
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().x, 50_000);
        assert_eq!(
            flex.child_rect(&ChildKey::from("two")).unwrap().width,
            15_535
        );
    }

    #[test]
    fn flex_large_fill_weights_do_not_overflow() {
        let mut flex = Flex::row()
            .child("one", Probe::default(), FlexItem::fill(u16::MAX))
            .child("two", Probe::default(), FlexItem::fill(u16::MAX))
            .child("three", Probe::default(), FlexItem::fill(u16::MAX));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 99, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().width, 33);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().width, 33);
        assert_eq!(flex.child_rect(&ChildKey::from("three")).unwrap().width, 33);
    }

    #[test]
    fn flex_gap_only_overflow_records_diagnostic() {
        let mut flex = Flex::row()
            .gap(2)
            .child("one", Probe::default(), FlexItem::fixed(0))
            .child("two", Probe::default(), FlexItem::fixed(0));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 1, 1), &mut ctx);

        assert_eq!(ctx.overflow_diagnostics().len(), 1);
        assert_eq!(ctx.overflow_diagnostics()[0].needed, 2);
        assert_eq!(ctx.overflow_diagnostics()[0].available, 1);
    }

    #[test]
    fn flex_fit_content_uses_measured_preferred_main_size() {
        let mut flex = Flex::row()
            .child(
                "content",
                MeasuredProbe {
                    min: LayoutSize::new(2, 1),
                    preferred: LayoutSize::new(8, 1),
                },
                FlexItem::fit_content(),
            )
            .child("fill", Probe::default(), FlexItem::fill(1));
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 20, 1), &mut ctx);

        assert_eq!(
            flex.child_rect(&ChildKey::from("content")).unwrap().width,
            8
        );
        assert_eq!(flex.child_rect(&ChildKey::from("fill")).unwrap().width, 12);
    }

    #[test]
    fn flex_fit_content_legacy_child_uses_visible_fallback() {
        let mut flex = Flex::row().child("legacy", Probe::default(), FlexItem::content());
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 20, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("legacy")).unwrap().width, 1);
    }

    #[test]
    fn flex_shrink_fit_content_distributes_fairly_in_rounds() {
        let mut flex = Flex::row()
            .child(
                "one",
                MeasuredProbe {
                    min: LayoutSize::new(6, 1),
                    preferred: LayoutSize::new(10, 1),
                },
                FlexItem::fit_content(),
            )
            .child(
                "two",
                MeasuredProbe {
                    min: LayoutSize::new(6, 1),
                    preferred: LayoutSize::new(8, 1),
                },
                FlexItem::fit_content(),
            );
        let mut ctx = LayoutCtx::new();

        flex.layout(Rect::new(0, 0, 13, 1), &mut ctx);

        assert_eq!(flex.child_rect(&ChildKey::from("one")).unwrap().width, 7);
        assert_eq!(flex.child_rect(&ChildKey::from("two")).unwrap().width, 6);
    }
}
