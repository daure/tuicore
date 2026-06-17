use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use super::stack::StackAlign;
use crate::spacing::{Gap, Padding};
use crate::{
    AnimationSettings, AxisProposal, ChildKey, Children, DuplicateChildKey, EventCtx, EventOutcome,
    EventRoute, FocusCtx, FocusTarget, HintSource, LayoutAxis, LayoutCtx, LayoutProposal,
    LayoutResult, LayoutSize, LayoutSizeHint, LifecycleCtx, MissingChildKey, OverflowPolicyName,
    TickResult, TuiEvent, TuiNode,
};
use crate::{GridSeparators, Separator, separator};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridTrack {
    Fixed(u16),
    Percent(u16),
    Fill(u16),
    FitContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridItem {
    pub row: usize,
    pub column: usize,
    pub row_span: usize,
    pub column_span: usize,
    pub horizontal: StackAlign,
    pub vertical: StackAlign,
}

pub struct Grid<M = ()> {
    columns: Vec<GridTrack>,
    rows: Vec<GridTrack>,
    children: Children<M>,
    items: Vec<GridChild>,
    rects: Vec<(ChildKey, Rect)>,
    cell_rects: Vec<Rect>,
    resolved_columns: Vec<u16>,
    resolved_rows: Vec<u16>,
    column_gap: u16,
    row_gap: u16,
    padding: Padding,
    separators: Option<GridSeparators>,
    column_separator_rects: Vec<Rect>,
    row_separator_rects: Vec<Rect>,
    intersections: Vec<(u16, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GridChild {
    key: ChildKey,
    item: GridItem,
}

impl GridTrack {
    pub fn fixed(size: u16) -> Self {
        Self::Fixed(size)
    }

    pub fn percent(percent: u16) -> Self {
        Self::Percent(percent.min(100))
    }

    pub fn fill(weight: u16) -> Self {
        Self::Fill(weight.max(1))
    }

    pub fn fit_content() -> Self {
        Self::FitContent
    }
}

impl GridItem {
    pub fn new(row: usize, column: usize) -> Self {
        Self {
            row,
            column,
            row_span: 1,
            column_span: 1,
            horizontal: StackAlign::Stretch,
            vertical: StackAlign::Stretch,
        }
    }

    pub fn span(mut self, rows: usize, columns: usize) -> Self {
        self.row_span = rows.max(1);
        self.column_span = columns.max(1);
        self
    }

    pub fn align(mut self, horizontal: StackAlign, vertical: StackAlign) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }
}

impl<M> Default for Grid<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M> Grid<M> {
    pub fn new() -> Self {
        Self {
            columns: vec![GridTrack::Fill(1)],
            rows: vec![GridTrack::Fill(1)],
            children: Children::new(),
            items: Vec::new(),
            rects: Vec::new(),
            cell_rects: Vec::new(),
            resolved_columns: Vec::new(),
            resolved_rows: Vec::new(),
            column_gap: 0,
            row_gap: 0,
            padding: Padding::default(),
            separators: None,
            column_separator_rects: Vec::new(),
            row_separator_rects: Vec::new(),
            intersections: Vec::new(),
        }
    }

    pub fn columns(mut self, columns: impl Into<Vec<GridTrack>>) -> Self {
        self.columns = non_empty(columns.into());
        self
    }

    pub fn rows(mut self, rows: impl Into<Vec<GridTrack>>) -> Self {
        self.rows = non_empty(rows.into());
        self
    }

    pub fn gap(mut self, columns: u16, rows: u16) -> Self {
        self.column_gap = columns;
        self.row_gap = rows;
        self
    }

    pub fn gaps(mut self, gaps: Gap) -> Self {
        self.column_gap = gaps.column;
        self.row_gap = gaps.row;
        self
    }

    pub fn padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self
    }

    pub fn separators(mut self, separators: GridSeparators) -> Self {
        self.separators = Some(separators);
        self
    }

    pub fn separator(mut self, separator: Separator) -> Self {
        self.separators = Some(GridSeparators::both(separator));
        self
    }

    pub fn child_rect(&self, key: &ChildKey) -> Option<Rect> {
        self.rects
            .iter()
            .find_map(|(child_key, rect)| (child_key == key).then_some(*rect))
    }

    fn is_spanned(&self, x: u16, y: u16) -> bool {
        self.cell_rects.iter().any(|rect| {
            x >= rect.x
                && x < rect.x.saturating_add(rect.width)
                && y >= rect.y
                && y < rect.y.saturating_add(rect.height)
        })
    }

    fn grid_occupancy(&self) -> Vec<Vec<Option<usize>>> {
        let mut occupancy = vec![vec![None; self.resolved_columns.len()]; self.resolved_rows.len()];
        for (item_index, child) in self.items.iter().enumerate() {
            let col = child
                .item
                .column
                .min(self.resolved_columns.len().saturating_sub(1));
            let row = child
                .item
                .row
                .min(self.resolved_rows.len().saturating_sub(1));
            let col_end = col
                .saturating_add(child.item.column_span)
                .min(self.resolved_columns.len());
            let row_end = row
                .saturating_add(child.item.row_span)
                .min(self.resolved_rows.len());
            for r in row..row_end {
                for c in col..col_end {
                    occupancy[r][c] = Some(item_index);
                }
            }
        }
        occupancy
    }

    fn has_column_boundary(&self, occupancy: &[Vec<Option<usize>>], r: usize, c: usize) -> bool {
        if r >= occupancy.len() || c + 1 >= self.resolved_columns.len() {
            return true;
        }
        match (occupancy[r][c], occupancy[r][c + 1]) {
            (Some(i), Some(j)) => i != j,
            _ => true,
        }
    }

    fn has_row_boundary(&self, occupancy: &[Vec<Option<usize>>], r: usize, c: usize) -> bool {
        if r + 1 >= self.resolved_rows.len() || c >= self.resolved_columns.len() {
            return true;
        }
        match (occupancy[r][c], occupancy[r + 1][c]) {
            (Some(i), Some(j)) => i != j,
            _ => true,
        }
    }
}

impl<M> Grid<M>
where
    M: 'static,
{
    pub fn child<C>(mut self, key: impl Into<ChildKey>, child: C, item: GridItem) -> Self
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
        item: GridItem,
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
        item: GridItem,
    ) -> Result<(), DuplicateChildKey>
    where
        C: TuiNode<M> + 'static,
    {
        let key = key.into();
        self.children = std::mem::take(&mut self.children).try_child(key.clone(), child)?;
        self.items.push(GridChild { key, item });
        Ok(())
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

impl<M> TuiNode<M> for Grid<M> {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        let padding_width = self.padding.left.saturating_add(self.padding.right);
        let padding_height = self.padding.top.saturating_add(self.padding.bottom);
        let columns = self.measure_tracks(&self.columns, proposal.width, padding_width, true);
        let rows = self.measure_tracks(&self.rows, proposal.height, padding_height, false);
        let width = sum_with_gaps(&columns, self.track_spacing(true)).saturating_add(padding_width);
        let height = sum_with_gaps(&rows, self.track_spacing(false)).saturating_add(padding_height);
        let min_columns =
            self.measure_min_tracks(&self.columns, proposal.width, padding_width, true);
        let min_rows = self.measure_min_tracks(&self.rows, proposal.height, padding_height, false);
        let min_width =
            sum_with_gaps(&min_columns, self.track_spacing(true)).saturating_add(padding_width);
        let min_height =
            sum_with_gaps(&min_rows, self.track_spacing(false)).saturating_add(padding_height);
        LayoutSizeHint {
            source: HintSource::Measured,
            min: LayoutSize::new(min_width, min_height),
            preferred: LayoutSize::new(width, height),
            expand: crate::AxisExpand {
                width: self
                    .columns
                    .iter()
                    .any(|track| matches!(track, GridTrack::Fill(_))),
                height: self
                    .rows
                    .iter()
                    .any(|track| matches!(track, GridTrack::Fill(_))),
            },
        }
        .normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let inner = inner_area(area, self.padding);
        let columns = self.resolve_tracks(&self.columns, inner.width, true);
        let rows = self.resolve_tracks(&self.rows, inner.height, false);
        record_track_overflow(
            ctx,
            LayoutAxis::Width,
            &columns,
            self.track_spacing(true),
            inner.width,
        );
        record_track_overflow(
            ctx,
            LayoutAxis::Height,
            &rows,
            self.track_spacing(false),
            inner.height,
        );
        self.rects = self.calculate_rects(inner, &columns, &rows);
        self.cell_rects = self
            .items
            .iter()
            .map(|child| {
                cell_rect(
                    inner,
                    &columns,
                    &rows,
                    self.track_spacing(true),
                    self.track_spacing(false),
                    child.item,
                )
            })
            .collect();
        self.resolved_columns = columns.clone();
        self.resolved_rows = rows.clone();
        self.column_separator_rects = self.column_separator_rects(inner, &columns);
        self.row_separator_rects = self.row_separator_rects(inner, &rows);
        self.intersections = self.intersections();
        for (key, rect) in &self.rects {
            self.children.layout_child(key, *rect, ctx);
        }
        LayoutResult::new(area)
    }

    fn render(&self, frame: &mut Frame, _area: Rect) {
        for (key, rect) in &self.rects {
            if let Some(child) = self.children.get(key) {
                child.render(frame, *rect);
            }
        }
        if let Some(separators) = self.separators {
            let separator = separators.separator();
            let border = crate::border_chars(separator.resolved_kind());
            let occupancy = self.grid_occupancy();

            let inner = inner_area(_area, self.padding);
            let mut col_starts = Vec::new();
            let mut col_ends = Vec::new();
            let mut current_x = inner.x;
            for &w in &self.resolved_columns {
                col_starts.push(current_x);
                col_ends.push(current_x.saturating_add(w));
                current_x = current_x
                    .saturating_add(w)
                    .saturating_add(self.track_spacing(true));
            }

            let mut row_starts = Vec::new();
            let mut row_ends = Vec::new();
            let mut current_y = inner.y;
            for &h in &self.resolved_rows {
                row_starts.push(current_y);
                row_ends.push(current_y.saturating_add(h));
                current_y = current_y
                    .saturating_add(h)
                    .saturating_add(self.track_spacing(false));
            }

            for rect in &self.column_separator_rects {
                let style = separator.style();
                for y in rect.y..rect.y.saturating_add(rect.height) {
                    for x in rect.x..rect.x.saturating_add(rect.width) {
                        if self.is_spanned(x, y) {
                            continue;
                        }
                        let col_loc = find_track_location(x, &col_starts, &col_ends);
                        let row_loc = find_track_location(y, &row_starts, &row_ends);
                        if let TrackLocation::Gap(c) = col_loc {
                            let draw = match row_loc {
                                TrackLocation::Inside(r) => {
                                    self.has_column_boundary(&occupancy, r, c)
                                }
                                TrackLocation::Gap(r) => {
                                    if let Some(y_s) =
                                        self.row_separator_rects.get(r).map(|rect| rect.y)
                                    {
                                        if y < y_s {
                                            self.has_column_boundary(&occupancy, r, c)
                                        } else if y > y_s {
                                            self.has_column_boundary(&occupancy, r + 1, c)
                                        } else {
                                            self.has_column_boundary(&occupancy, r, c)
                                                || self.has_column_boundary(&occupancy, r + 1, c)
                                        }
                                    } else {
                                        self.has_column_boundary(&occupancy, r, c)
                                            && self.has_column_boundary(&occupancy, r + 1, c)
                                    }
                                }
                                TrackLocation::Outside => {
                                    if y < row_starts.first().copied().unwrap_or(0) {
                                        self.has_column_boundary(&occupancy, 0, c)
                                    } else {
                                        let last_row = occupancy.len().saturating_sub(1);
                                        self.has_column_boundary(&occupancy, last_row, c)
                                    }
                                }
                            };
                            if draw {
                                frame.buffer_mut().set_string(x, y, border.vertical, style);
                            }
                        }
                    }
                }
            }

            for rect in &self.row_separator_rects {
                let style = separator.style();
                for y in rect.y..rect.y.saturating_add(rect.height) {
                    for x in rect.x..rect.x.saturating_add(rect.width) {
                        if self.is_spanned(x, y) {
                            continue;
                        }
                        let col_loc = find_track_location(x, &col_starts, &col_ends);
                        let row_loc = find_track_location(y, &row_starts, &row_ends);
                        if let TrackLocation::Gap(r) = row_loc {
                            let draw = match col_loc {
                                TrackLocation::Inside(c) => self.has_row_boundary(&occupancy, r, c),
                                TrackLocation::Gap(c) => {
                                    if let Some(x_s) =
                                        self.column_separator_rects.get(c).map(|rect| rect.x)
                                    {
                                        if x < x_s {
                                            self.has_row_boundary(&occupancy, r, c)
                                        } else if x > x_s {
                                            self.has_row_boundary(&occupancy, r, c + 1)
                                        } else {
                                            self.has_row_boundary(&occupancy, r, c)
                                                || self.has_row_boundary(&occupancy, r, c + 1)
                                        }
                                    } else {
                                        self.has_row_boundary(&occupancy, r, c)
                                            && self.has_row_boundary(&occupancy, r, c + 1)
                                    }
                                }
                                TrackLocation::Outside => {
                                    if x < col_starts.first().copied().unwrap_or(0) {
                                        self.has_row_boundary(&occupancy, r, 0)
                                    } else {
                                        let last_col =
                                            self.resolved_columns.len().saturating_sub(1);
                                        self.has_row_boundary(&occupancy, r, last_col)
                                    }
                                }
                            };
                            if draw {
                                frame
                                    .buffer_mut()
                                    .set_string(x, y, border.horizontal, style);
                            }
                        }
                    }
                }
            }

            if separators.axes().has_columns() && separators.axes().has_rows() {
                for &(x, y) in &self.intersections {
                    if self.is_spanned(x, y) {
                        continue;
                    }
                    let col_loc = find_track_location(x, &col_starts, &col_ends);
                    let row_loc = find_track_location(y, &row_starts, &row_ends);
                    if let (TrackLocation::Gap(c), TrackLocation::Gap(r)) = (col_loc, row_loc) {
                        let top = self.has_column_boundary(&occupancy, r, c);
                        let bottom = self.has_column_boundary(&occupancy, r + 1, c);
                        let left = self.has_row_boundary(&occupancy, r, c);
                        let right = self.has_row_boundary(&occupancy, r, c + 1);

                        let sym = junction_char(
                            border,
                            separator::cross(separator.resolved_kind()),
                            top,
                            bottom,
                            left,
                            right,
                        );
                        frame.buffer_mut().set_string(x, y, sym, separator.style());
                    }
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
        self.children
            .dispatch_routed_child(route, event, ctx)
            .bubble(ctx, |ctx| self.event(event, ctx))
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

impl<M> Grid<M> {
    fn measure_tracks(
        &self,
        tracks: &[GridTrack],
        proposal: AxisProposal,
        padding: u16,
        columns: bool,
    ) -> Vec<u16> {
        match proposal {
            AxisProposal::AtMost(available) | AxisProposal::Exact(available) => {
                self.resolve_tracks(tracks, available.saturating_sub(padding), columns)
            }
            AxisProposal::Unbounded => tracks
                .iter()
                .enumerate()
                .map(|(index, track)| match *track {
                    GridTrack::Fixed(size) => size,
                    GridTrack::FitContent => self.fit_content_track(index, columns, u16::MAX),
                    GridTrack::Percent(_) | GridTrack::Fill(_) => 0,
                })
                .collect(),
        }
    }

    fn min_fit_content_track(&self, index: usize, columns: bool, available: u16) -> u16 {
        self.items
            .iter()
            .filter(|child| single_span_on_track(child, index, columns))
            .filter_map(|child| {
                self.children
                    .measure_child(&child.key, LayoutProposal::at_most(available, available))
            })
            .map(|hint| {
                if hint.source == HintSource::LegacyUnmeasured {
                    0
                } else if columns {
                    hint.min.width
                } else {
                    hint.min.height
                }
            })
            .max()
            .unwrap_or(0)
            .min(available)
    }

    fn measure_min_tracks(
        &self,
        tracks: &[GridTrack],
        proposal: AxisProposal,
        padding: u16,
        columns: bool,
    ) -> Vec<u16> {
        let available = match proposal {
            AxisProposal::AtMost(value) | AxisProposal::Exact(value) => {
                value.saturating_sub(padding)
            }
            AxisProposal::Unbounded => u16::MAX,
        };
        tracks
            .iter()
            .enumerate()
            .map(|(index, track)| match *track {
                GridTrack::Fixed(size) => size,
                GridTrack::FitContent => self.min_fit_content_track(index, columns, available),
                GridTrack::Percent(_) | GridTrack::Fill(_) => 0,
            })
            .collect()
    }

    fn resolve_tracks(&self, tracks: &[GridTrack], available: u16, columns: bool) -> Vec<u16> {
        let gap = if columns {
            self.column_gap
        } else {
            self.row_gap
        };
        let gap_total = total_gap(tracks.len(), gap).saturating_add(separator::separator_slots(
            self.axis_has_separator(columns),
            tracks.len(),
        ));
        let available_without_gap = u32::from(available).saturating_sub(u32::from(gap_total));
        let mut lengths = vec![0u32; tracks.len()];
        let mut fill_weight = 0u32;

        for (index, track) in tracks.iter().enumerate() {
            match *track {
                GridTrack::Fixed(size) => lengths[index] = u32::from(size),
                GridTrack::Percent(percent) => {
                    lengths[index] = available_without_gap.saturating_mul(u32::from(percent)) / 100;
                }
                GridTrack::Fill(weight) => {
                    fill_weight = fill_weight.saturating_add(u32::from(weight))
                }
                GridTrack::FitContent => {
                    lengths[index] = u32::from(self.fit_content_track(index, columns, available));
                }
            }
        }

        let reserved = lengths
            .iter()
            .fold(0u32, |sum, value| sum.saturating_add(*value));
        let fill_space = available_without_gap.saturating_sub(reserved);
        let mut distributed = 0u32;
        if fill_weight > 0 {
            for (index, track) in tracks.iter().enumerate() {
                if let GridTrack::Fill(weight) = *track {
                    let share = fill_space.saturating_mul(u32::from(weight)) / fill_weight;
                    lengths[index] = share;
                    distributed = distributed.saturating_add(share);
                }
            }
        }
        let mut remainder = fill_space.saturating_sub(distributed);
        for (index, track) in tracks.iter().enumerate() {
            if remainder == 0 {
                break;
            }
            if matches!(track, GridTrack::Fill(_)) {
                lengths[index] = lengths[index].saturating_add(1);
                remainder -= 1;
            }
        }

        lengths.into_iter().map(clamp_u32_to_u16).collect()
    }

    fn fit_content_track(&self, index: usize, columns: bool, available: u16) -> u16 {
        self.items
            .iter()
            .filter(|child| single_span_on_track(child, index, columns))
            .filter_map(|child| {
                self.children
                    .measure_child(&child.key, LayoutProposal::at_most(available, available))
            })
            .map(|hint| {
                if hint.source == HintSource::LegacyUnmeasured {
                    (available > 0) as u16
                } else if columns {
                    hint.preferred.width
                } else {
                    hint.preferred.height
                }
            })
            .max()
            .unwrap_or(0)
            .min(available)
    }

    fn calculate_rects(&self, area: Rect, columns: &[u16], rows: &[u16]) -> Vec<(ChildKey, Rect)> {
        self.items
            .iter()
            .map(|child| {
                let cell = cell_rect(
                    area,
                    columns,
                    rows,
                    self.track_spacing(true),
                    self.track_spacing(false),
                    child.item,
                );
                let hint = self.children.measure_child(
                    &child.key,
                    LayoutProposal {
                        width: AxisProposal::AtMost(cell.width),
                        height: AxisProposal::AtMost(cell.height),
                    },
                );
                let rect = aligned_rect(cell, child.item, hint);
                (child.key.clone(), rect)
            })
            .collect()
    }

    fn track_spacing(&self, columns: bool) -> u16 {
        let gap = if columns {
            self.column_gap
        } else {
            self.row_gap
        };
        gap.saturating_add(self.axis_has_separator(columns) as u16)
    }

    fn axis_has_separator(&self, columns: bool) -> bool {
        self.separators
            .map(|separators| {
                if columns {
                    separators.axes().has_columns()
                } else {
                    separators.axes().has_rows()
                }
            })
            .unwrap_or(false)
    }

    fn column_separator_rects(&self, area: Rect, columns: &[u16]) -> Vec<Rect> {
        if !self.axis_has_separator(true) {
            return Vec::new();
        }
        self.separator_offsets(columns, self.column_gap)
            .into_iter()
            .filter(|offset| *offset < area.width)
            .map(|offset| Rect::new(area.x.saturating_add(offset), area.y, 1, area.height))
            .collect()
    }

    fn row_separator_rects(&self, area: Rect, rows: &[u16]) -> Vec<Rect> {
        if !self.axis_has_separator(false) {
            return Vec::new();
        }
        self.separator_offsets(rows, self.row_gap)
            .into_iter()
            .filter(|offset| *offset < area.height)
            .map(|offset| Rect::new(area.x, area.y.saturating_add(offset), area.width, 1))
            .collect()
    }

    fn separator_offsets(&self, tracks: &[u16], gap: u16) -> Vec<u16> {
        let spacing = gap.saturating_add(1);
        (0..tracks.len().saturating_sub(1))
            .map(|index| {
                offset_before(tracks, index, spacing)
                    .saturating_add(tracks[index])
                    .saturating_add(gap / 2)
            })
            .collect()
    }

    fn intersections(&self) -> Vec<(u16, u16)> {
        self.column_separator_rects
            .iter()
            .flat_map(|column| {
                self.row_separator_rects
                    .iter()
                    .map(move |row| (column.x, row.y))
            })
            .collect()
    }
}

fn non_empty(mut tracks: Vec<GridTrack>) -> Vec<GridTrack> {
    if tracks.is_empty() {
        tracks.push(GridTrack::Fill(1));
    }
    tracks
}

fn inner_area(area: Rect, padding: Padding) -> Rect {
    let x = area.x.saturating_add(padding.left);
    let y = area.y.saturating_add(padding.top);
    let width = area
        .width
        .saturating_sub(padding.left.saturating_add(padding.right));
    let height = area
        .height
        .saturating_sub(padding.top.saturating_add(padding.bottom));
    Rect::new(x, y, width, height)
}

fn single_span_on_track(child: &GridChild, index: usize, columns: bool) -> bool {
    if columns {
        child.item.column == index && child.item.column_span == 1
    } else {
        child.item.row == index && child.item.row_span == 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrackLocation {
    Inside(usize),
    Gap(usize),
    Outside,
}

fn find_track_location(coord: u16, starts: &[u16], ends: &[u16]) -> TrackLocation {
    if starts.is_empty() {
        return TrackLocation::Outside;
    }
    if coord < starts[0] {
        return TrackLocation::Outside;
    }
    for i in 0..starts.len() {
        if coord >= starts[i] && coord < ends[i] {
            return TrackLocation::Inside(i);
        }
        if i + 1 < starts.len() && coord >= ends[i] && coord < starts[i + 1] {
            return TrackLocation::Gap(i);
        }
    }
    TrackLocation::Outside
}

fn junction_char(
    border: crate::border::BorderChars,
    cross: &'static str,
    top: bool,
    bottom: bool,
    left: bool,
    right: bool,
) -> &'static str {
    match (top, bottom, left, right) {
        (true, true, true, true) => cross,
        (true, true, true, false) => border.right_join,
        (true, true, false, true) => border.left_join,
        (true, true, false, false) => border.vertical,
        (true, false, true, true) => border.bottom_join,
        (true, false, true, false) => border.bottom_right,
        (true, false, false, true) => border.bottom_left,
        (true, false, false, false) => border.vertical,
        (false, true, true, true) => border.top_join,
        (false, true, true, false) => border.top_right,
        (false, true, false, true) => border.top_left,
        (false, true, false, false) => border.vertical,
        (false, false, true, true) => border.horizontal,
        (false, false, true, false) => border.horizontal,
        (false, false, false, true) => border.horizontal,
        (false, false, false, false) => " ",
    }
}

fn cell_rect(
    area: Rect,
    columns: &[u16],
    rows: &[u16],
    column_gap: u16,
    row_gap: u16,
    item: GridItem,
) -> Rect {
    let column = item.column.min(columns.len().saturating_sub(1));
    let row = item.row.min(rows.len().saturating_sub(1));
    let column_end = column.saturating_add(item.column_span).min(columns.len());
    let row_end = row.saturating_add(item.row_span).min(rows.len());
    let x = area
        .x
        .saturating_add(offset_before(columns, column, column_gap));
    let y = area.y.saturating_add(offset_before(rows, row, row_gap));
    let width = sum_span(&columns[column..column_end], column_gap);
    let height = sum_span(&rows[row..row_end], row_gap);
    Rect::new(x, y, width, height)
}

fn aligned_rect(cell: Rect, item: GridItem, hint: Option<LayoutSizeHint>) -> Rect {
    let width = align_size(cell.width, item.horizontal, hint, true);
    let height = align_size(cell.height, item.vertical, hint, false);
    let x = align_offset(cell.x, cell.width, width, item.horizontal);
    let y = align_offset(cell.y, cell.height, height, item.vertical);
    Rect::new(x, y, width, height)
}

fn align_size(available: u16, align: StackAlign, hint: Option<LayoutSizeHint>, width: bool) -> u16 {
    if align == StackAlign::Stretch {
        return available;
    }
    match hint {
        Some(hint) if hint.source == HintSource::Measured => if width {
            hint.preferred.width
        } else {
            hint.preferred.height
        }
        .min(available),
        _ => ((available > 0) as u16).min(available),
    }
}

fn align_offset(origin: u16, available: u16, size: u16, align: StackAlign) -> u16 {
    let slack = available.saturating_sub(size);
    origin.saturating_add(match align {
        StackAlign::Stretch | StackAlign::Start => 0,
        StackAlign::Center => slack / 2,
        StackAlign::End => slack,
    })
}

fn offset_before(lengths: &[u16], index: usize, gap: u16) -> u16 {
    sum_span(&lengths[..index], gap).saturating_add(if index == 0 { 0 } else { gap })
}

fn sum_with_gaps(lengths: &[u16], gap: u16) -> u16 {
    sum_span(lengths, gap)
}

fn sum_span(lengths: &[u16], gap: u16) -> u16 {
    let base = lengths
        .iter()
        .fold(0u16, |sum, value| sum.saturating_add(*value));
    base.saturating_add(total_gap(lengths.len(), gap))
}

fn total_gap(count: usize, gap: u16) -> u16 {
    gap.saturating_mul(count.saturating_sub(1).min(usize::from(u16::MAX)) as u16)
}

fn record_track_overflow(
    ctx: &mut LayoutCtx,
    axis: LayoutAxis,
    lengths: &[u16],
    gap: u16,
    available: u16,
) {
    let needed = sum_with_gaps(lengths, gap);
    if needed > available {
        ctx.record_overflow(axis, needed, available, OverflowPolicyName::Clip);
    }
}

fn clamp_u32_to_u16(value: u32) -> u16 {
    value.min(u32::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Probe {
        size: LayoutSize,
    }

    impl TuiNode<()> for Probe {
        fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
            LayoutSizeHint::content(self.size.width, self.size.height).normalized(proposal)
        }

        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    struct LegacyProbe;

    impl TuiNode<()> for LegacyProbe {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect) {}
    }

    #[test]
    fn grid_lays_out_mixed_tracks_with_fit_content() {
        let mut grid = Grid::new()
            .columns(vec![
                GridTrack::fixed(5),
                GridTrack::fit_content(),
                GridTrack::fill(1),
            ])
            .rows(vec![
                GridTrack::fixed(2),
                GridTrack::percent(50),
                GridTrack::fill(1),
            ])
            .gap(1, 1)
            .child(
                "fit",
                Probe {
                    size: LayoutSize::new(8, 1),
                },
                GridItem::new(0, 1),
            )
            .child(
                "fill",
                Probe {
                    size: LayoutSize::new(1, 1),
                },
                GridItem::new(2, 2),
            );
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 30, 12), &mut ctx);

        assert_eq!(
            grid.child_rect(&ChildKey::from("fit")),
            Some(Rect::new(6, 0, 8, 2))
        );
        assert_eq!(
            grid.child_rect(&ChildKey::from("fill")),
            Some(Rect::new(15, 9, 15, 3))
        );
    }

    #[test]
    fn grid_gaps_and_padding_offset_child_rects() {
        let mut grid = Grid::new()
            .columns([GridTrack::fixed(5), GridTrack::fixed(5)])
            .rows([GridTrack::fixed(2), GridTrack::fixed(2)])
            .gaps(Gap::new(1, 2))
            .padding(Padding {
                left: 1,
                right: 3,
                top: 2,
                bottom: 4,
            })
            .child(
                "cell",
                Probe {
                    size: LayoutSize::new(5, 2),
                },
                GridItem::new(1, 1),
            );
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 20, 12), &mut ctx);

        assert_eq!(
            grid.child_rect(&ChildKey::from("cell")),
            Some(Rect::new(8, 5, 5, 2))
        );
    }

    #[test]
    fn grid_measure_includes_gaps_and_padding() {
        let grid = Grid::<()>::new()
            .columns([GridTrack::fixed(5), GridTrack::fixed(5)])
            .rows([GridTrack::fixed(2), GridTrack::fixed(2)])
            .gaps(Gap::new(1, 2))
            .padding(Padding {
                left: 1,
                right: 3,
                top: 2,
                bottom: 4,
            });

        let hint = grid.measure(LayoutProposal::unbounded());

        assert_eq!(hint.preferred, LayoutSize::new(16, 11));
    }

    #[test]
    fn grid_measure_applies_fixed_percent_fill_and_fit_content_tracks() {
        let grid = Grid::new()
            .columns([
                GridTrack::fixed(5),
                GridTrack::fit_content(),
                GridTrack::fill(1),
            ])
            .rows([
                GridTrack::fixed(2),
                GridTrack::percent(50),
                GridTrack::fill(1),
            ])
            .gap(1, 1)
            .child(
                "fit",
                Probe {
                    size: LayoutSize::new(8, 1),
                },
                GridItem::new(0, 1),
            );

        let hint = grid.measure(LayoutProposal::at_most(30, 12));

        assert_eq!(hint.preferred, LayoutSize::new(30, 12));
    }

    #[test]
    fn grid_measure_unbounded_fit_content_uses_child_or_legacy_fallback() {
        let grid = Grid::new()
            .columns([GridTrack::fit_content(), GridTrack::fit_content()])
            .rows([GridTrack::fit_content()])
            .gap(1, 0)
            .child(
                "measured",
                Probe {
                    size: LayoutSize::new(8, 2),
                },
                GridItem::new(0, 0),
            )
            .child("legacy", LegacyProbe, GridItem::new(0, 1));

        let hint = grid.measure(LayoutProposal::unbounded());

        assert_eq!(hint.preferred, LayoutSize::new(10, 2));
    }

    #[test]
    fn grid_column_separators_offset_column_tracks() {
        let mut grid = Grid::new()
            .columns([GridTrack::fixed(5), GridTrack::fixed(5)])
            .rows([GridTrack::fixed(2)])
            .separators(GridSeparators::columns(Separator::new()))
            .child(
                "right",
                Probe {
                    size: LayoutSize::new(5, 2),
                },
                GridItem::new(0, 1),
            );
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 11, 2), &mut ctx);

        assert_eq!(grid.child_rect(&ChildKey::from("right")).unwrap().x, 6);
        assert_eq!(grid.column_separator_rects, vec![Rect::new(5, 0, 1, 2)]);
    }

    #[test]
    fn grid_row_separators_offset_row_tracks() {
        let mut grid = Grid::new()
            .columns([GridTrack::fixed(5)])
            .rows([GridTrack::fixed(2), GridTrack::fixed(2)])
            .separators(GridSeparators::rows(Separator::new()))
            .child(
                "bottom",
                Probe {
                    size: LayoutSize::new(5, 2),
                },
                GridItem::new(1, 0),
            );
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 5, 5), &mut ctx);

        assert_eq!(grid.child_rect(&ChildKey::from("bottom")).unwrap().y, 3);
        assert_eq!(grid.row_separator_rects, vec![Rect::new(0, 2, 5, 1)]);
    }

    #[test]
    fn grid_both_separators_records_intersections() {
        let mut grid = Grid::<()>::new()
            .columns([GridTrack::fixed(2), GridTrack::fixed(2)])
            .rows([GridTrack::fixed(1), GridTrack::fixed(1)])
            .separator(Separator::new());
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 5, 3), &mut ctx);

        assert_eq!(grid.column_separator_rects, vec![Rect::new(2, 0, 1, 3)]);
        assert_eq!(grid.row_separator_rects, vec![Rect::new(0, 1, 5, 1)]);
        assert_eq!(grid.intersections, vec![(2, 1)]);
    }

    #[test]
    fn grid_measure_includes_separator_space() {
        let grid = Grid::<()>::new()
            .columns([GridTrack::fixed(5), GridTrack::fixed(5)])
            .rows([GridTrack::fixed(2), GridTrack::fixed(2)])
            .separator(Separator::new());

        let hint = grid.measure(LayoutProposal::unbounded());

        assert_eq!(hint.preferred, LayoutSize::new(11, 5));
    }

    #[test]
    fn grid_separators_handle_tiny_areas() {
        let mut grid = Grid::<()>::new()
            .columns([GridTrack::fill(1), GridTrack::fill(1)])
            .rows([GridTrack::fill(1), GridTrack::fill(1)])
            .separator(Separator::new());
        let mut ctx = LayoutCtx::new();

        grid.layout(Rect::new(0, 0, 1, 1), &mut ctx);

        assert_eq!(grid.column_separator_rects, vec![Rect::new(0, 0, 1, 1)]);
        assert_eq!(grid.row_separator_rects, vec![Rect::new(0, 0, 1, 1)]);
    }

    #[test]
    fn grid_separators_are_not_rendered_over_spanning_items() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut grid = Grid::<()>::new()
            .columns([GridTrack::fixed(2), GridTrack::fixed(2)])
            .rows([GridTrack::fixed(1)])
            .separators(GridSeparators::columns(Separator::new()))
            .child(
                "span",
                Probe {
                    size: LayoutSize::new(5, 1),
                },
                GridItem::new(0, 0).span(1, 2),
            );

        let mut ctx = LayoutCtx::new();
        grid.layout(Rect::new(0, 0, 5, 1), &mut ctx);

        let mut terminal = Terminal::new(TestBackend::new(5, 1)).expect("terminal should build");
        terminal
            .draw(|frame| grid.render(frame, frame.area()))
            .expect("grid should render");

        let buffer = terminal.backend().buffer();
        // Separator would be at x=2. Since the child spans both columns, x=2 should be empty/space, not '│'.
        assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn grid_separators_are_rendered_between_non_spanning_items() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut grid = Grid::<()>::new()
            .columns([GridTrack::fixed(2), GridTrack::fixed(2)])
            .rows([GridTrack::fixed(1)])
            .separators(GridSeparators::columns(Separator::new()))
            .child(
                "left",
                Probe {
                    size: LayoutSize::new(2, 1),
                },
                GridItem::new(0, 0),
            )
            .child(
                "right",
                Probe {
                    size: LayoutSize::new(2, 1),
                },
                GridItem::new(0, 1),
            );

        let mut ctx = LayoutCtx::new();
        grid.layout(Rect::new(0, 0, 5, 1), &mut ctx);

        let mut terminal = Terminal::new(TestBackend::new(5, 1)).expect("terminal should build");
        terminal
            .draw(|frame| grid.render(frame, frame.area()))
            .expect("grid should render");

        let buffer = terminal.backend().buffer();
        // Separator is at x=2. Since items do not span, the vertical separator character '│' should be rendered.
        assert_eq!(buffer.cell((2, 0)).unwrap().symbol(), "│");
    }

    #[test]
    fn grid_measure_computes_correct_min_size() {
        struct MinProbe {
            min: LayoutSize,
            preferred: LayoutSize,
        }
        impl TuiNode<()> for MinProbe {
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
            fn render(&self, _frame: &mut Frame, _area: Rect) {}
        }

        let grid = Grid::new()
            .columns([
                GridTrack::fixed(5),
                GridTrack::fit_content(),
                GridTrack::fill(1),
            ])
            .rows([
                GridTrack::fixed(2),
                GridTrack::fit_content(),
                GridTrack::fill(1),
            ])
            .gap(1, 1)
            .padding(Padding::all(1))
            .child(
                "fit",
                MinProbe {
                    min: LayoutSize::new(4, 2),
                    preferred: LayoutSize::new(8, 4),
                },
                GridItem::new(1, 1),
            );

        // Columns min width: fixed (5) + gap (1) + fit-content min (4) + gap (1) + fill (0) = 11. Plus padding (2) = 13.
        // Rows min height: fixed (2) + gap (1) + fit-content min (2) + gap (1) + fill (0) = 6. Plus padding (2) = 8.
        let hint = grid.measure(LayoutProposal::unbounded());
        assert_eq!(hint.min, LayoutSize::new(13, 8));
        assert_eq!(hint.preferred, LayoutSize::new(17, 10)); // preferred: columns = 5+1+8+1+0 = 15 + padding 2 = 17. rows = 2+1+4+1+0 = 8 + padding 2 = 10.
    }

    #[test]
    fn debug_gallery_grid_rendering() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let mut grid = Grid::<()>::new()
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
            .separator(Separator::new())
            .padding(Padding::all(1))
            .child(
                "filters",
                Probe {
                    size: LayoutSize::new(10, 3),
                },
                GridItem::new(0, 0),
            )
            .child(
                "summary",
                Probe {
                    size: LayoutSize::new(18, 3),
                },
                GridItem::new(0, 1),
            )
            .child(
                "chart",
                Probe {
                    size: LayoutSize::new(28, 8),
                },
                GridItem::new(0, 2).span(2, 1),
            )
            .child(
                "table",
                Probe {
                    size: LayoutSize::new(30, 8),
                },
                GridItem::new(1, 0).span(2, 2),
            );

        let mut ctx = LayoutCtx::new();
        grid.layout(Rect::new(0, 0, 120, 40), &mut ctx);

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("terminal should build");
        terminal
            .draw(|frame| grid.render(frame, frame.area()))
            .expect("grid should render");

        let buffer = terminal.backend().buffer();
        // The horizontal separator at y=5 between Row 0 and Row 1 should extend through the gap to meet the vertical separator at x=37.
        assert_eq!(buffer.cell((36, 5)).unwrap().symbol(), "─");
        assert_eq!(buffer.cell((37, 5)).unwrap().symbol(), "┤");
    }
}
