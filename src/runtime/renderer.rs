use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    style::{Color, Style},
};

use crate::{OverlayLayer, RenderCtx, ToastRack, TuiNode, fade_buffer, theme};

use super::{
    Result,
    mouse_copy::{CellSelection, apply_selection},
};

pub(crate) const BASE_DIRECT_KITTY_Z_INDEX: i32 = -1_000_000_000;
const KITTY_LAYER_SPAN: i32 = 190_000_000;
const KITTY_OVERLAY_Z_MIN: i32 = -5_000;
const KITTY_OVERLAY_Z_MAX: i32 = 4_999;
const KITTY_Z_BUCKET_SPAN: i32 = 19_000;
const KITTY_ORDER_MAX: u64 = 9_499;

pub(crate) fn next_direct_kitty_image_id() -> u32 {
    static NEXT_IMAGE_ID: AtomicU32 = AtomicU32::new(1);
    NEXT_IMAGE_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Default)]
pub struct Renderer {
    direct_kitty: DirectKittyGraphics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct DirectKittyPlacementId {
    pub image_id: u32,
    pub placement_id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct GraphicsLevel {
    portal: bool,
    layer: OverlayLayer,
    z_index: i32,
    order: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct DirectKittyIntent {
    pub id: DirectKittyPlacementId,
    pub area: Rect,
    pub source_rect: DirectKittySourceRect,
    pub generation: u64,
    pub payload: Arc<str>,
    pub level: GraphicsLevel,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirectKittySourceRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct DirectKittyResident {
    pub image_id: u32,
    pub payload: Arc<str>,
}

#[derive(Debug, Default)]
pub(crate) struct GraphicsFrame {
    pub residents: Vec<DirectKittyResident>,
    pub intents: Vec<DirectKittyIntent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DirectKittyCommand {
    DeletePlacement(DirectKittyPlacementId),
    DeleteImage(u32),
    Transmit {
        image_id: u32,
        payload: Arc<str>,
    },
    Place {
        id: DirectKittyPlacementId,
        area: Rect,
        source_rect: DirectKittySourceRect,
        z_index: i32,
    },
}

#[derive(Debug, Default)]
struct DirectKittyGraphics {
    active: BTreeMap<DirectKittyPlacementId, DirectKittyIntent>,
    transmitted: BTreeMap<u32, Arc<str>>,
}

impl GraphicsLevel {
    pub(crate) const fn base() -> Self {
        Self {
            portal: false,
            layer: OverlayLayer::Popup,
            z_index: 0,
            order: 0,
        }
    }

    pub(crate) const fn new(layer: OverlayLayer, z_index: i32, order: u64) -> Self {
        Self {
            portal: true,
            layer,
            z_index,
            order,
        }
    }

    pub(crate) fn kitty_image_z_index(self) -> i32 {
        if !self.portal {
            return BASE_DIRECT_KITTY_Z_INDEX;
        }

        let layer = match self.layer {
            OverlayLayer::Popup => 0,
            OverlayLayer::Popover => 1,
            OverlayLayer::Modal => 2,
            OverlayLayer::Tooltip => 3,
            OverlayLayer::System => 4,
        };
        let overlay_z =
            self.z_index.clamp(KITTY_OVERLAY_Z_MIN, KITTY_OVERLAY_Z_MAX) - KITTY_OVERLAY_Z_MIN;
        let order = self.order.min(KITTY_ORDER_MAX) as i32;
        BASE_DIRECT_KITTY_Z_INDEX
            + 2
            + layer * KITTY_LAYER_SPAN
            + overlay_z * KITTY_Z_BUCKET_SPAN
            + order * 2
    }
}

impl DirectKittyGraphics {
    fn reconcile(&mut self, frame: GraphicsFrame) -> Vec<DirectKittyCommand> {
        let residents = frame
            .residents
            .into_iter()
            .map(|resident| (resident.image_id, resident.payload))
            .collect::<BTreeMap<_, _>>();
        let mut desired = BTreeMap::new();
        let mut display_order = Vec::new();
        for intent in frame.intents {
            if desired.insert(intent.id, intent.clone()).is_none() {
                display_order.push(intent.id);
            }
        }
        display_order.sort_by_key(|id| {
            let intent = desired
                .get(id)
                .expect("display order only contains registered Kitty placements");
            (intent.z_index, intent.level)
        });
        let visible_image_ids = desired
            .keys()
            .map(|id| id.image_id)
            .collect::<BTreeSet<_>>();

        let changed_image_ids = residents
            .iter()
            .filter(|(image_id, payload)| {
                self.transmitted
                    .get(image_id)
                    .is_some_and(|transmitted| transmitted != *payload)
            })
            .map(|(image_id, _)| *image_id)
            .collect::<BTreeSet<_>>();

        let mut commands = Vec::new();
        let mut deleted_images = BTreeSet::new();
        let transmitted_image_ids = self.transmitted.keys().copied().collect::<Vec<_>>();
        for image_id in transmitted_image_ids {
            if (!residents.contains_key(&image_id) || changed_image_ids.contains(&image_id))
                && deleted_images.insert(image_id)
            {
                commands.push(DirectKittyCommand::DeleteImage(image_id));
                self.transmitted.remove(&image_id);
            }
        }
        for (id, active) in &self.active {
            if desired.get(id) == Some(active) {
                continue;
            }
            if !changed_image_ids.contains(&id.image_id)
                && residents.contains_key(&id.image_id)
                && !desired.contains_key(id)
            {
                commands.push(DirectKittyCommand::DeletePlacement(*id));
            }
        }
        for (image_id, payload) in &residents {
            if visible_image_ids.contains(image_id)
                && self.transmitted.get(image_id) != Some(payload)
            {
                commands.push(DirectKittyCommand::Transmit {
                    image_id: *image_id,
                    payload: Arc::clone(payload),
                });
                self.transmitted.insert(*image_id, Arc::clone(payload));
            }
        }
        for id in display_order {
            let intent = desired
                .get(&id)
                .expect("display order only contains registered Kitty placements");
            if self.active.get(&id) != Some(intent) || changed_image_ids.contains(&id.image_id) {
                commands.push(DirectKittyCommand::Place {
                    id,
                    area: intent.area,
                    source_rect: intent.source_rect,
                    z_index: intent.z_index,
                });
            }
        }
        self.active = desired;
        commands
    }

    fn clear(&mut self) -> Vec<DirectKittyCommand> {
        let commands = self
            .transmitted
            .keys()
            .copied()
            .map(DirectKittyCommand::DeleteImage)
            .collect();
        self.active.clear();
        self.transmitted.clear();
        commands
    }
}

impl PartialEq for DirectKittyIntent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.area == other.area
            && self.source_rect == other.source_rect
            && self.generation == other.generation
            && self.z_index == other.z_index
    }
}

impl Eq for DirectKittyIntent {}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn render<B, N, M>(
        &mut self,
        terminal: &mut Terminal<B>,
        root: &N,
        area: Rect,
    ) -> Result<()>
    where
        B: Backend,
        N: TuiNode<M>,
        std::io::Error: From<B::Error>,
    {
        terminal.draw(|frame| {
            render_frame(frame, root, area);
        })?;
        Ok(())
    }

    pub fn render_with_toasts<B, N, M>(
        &mut self,
        terminal: &mut Terminal<B>,
        root: &N,
        toasts: &ToastRack,
        area: Rect,
    ) -> Result<()>
    where
        B: Backend,
        N: TuiNode<M>,
        std::io::Error: From<B::Error>,
    {
        terminal.draw(|frame| {
            render_frame_with_toasts_and_fade(frame, root, toasts, area, 0.0);
        })?;
        Ok(())
    }

    pub(crate) fn render_with_toasts_and_fade_to_crossterm<W, N, M>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
        root: &N,
        toasts: &ToastRack,
        area: Rect,
        fade_amount: f64,
        selection: Option<CellSelection>,
    ) -> Result<ratatui::buffer::Buffer>
    where
        W: Write,
        N: TuiNode<M>,
    {
        let mut rendered_buffer = None;
        let graphics = draw_frame(terminal, |frame| {
            let graphics =
                render_frame_with_toasts_and_fade(frame, root, toasts, area, fade_amount);
            rendered_buffer = Some(frame.buffer_mut().clone());
            if let Some(selection) = selection {
                apply_selection(frame.buffer_mut(), selection);
            }
            graphics
        })?;
        emit_direct_kitty(
            terminal.backend_mut(),
            self.direct_kitty.reconcile(graphics),
        )?;
        Ok(rendered_buffer.expect("draw callback should capture the rendered buffer"))
    }

    pub(crate) fn clear_direct_kitty<W>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
    ) -> Result<()>
    where
        W: Write,
    {
        emit_direct_kitty(terminal.backend_mut(), self.direct_kitty.clear())
    }
}

fn draw_frame<B>(
    terminal: &mut Terminal<B>,
    render: impl FnOnce(&mut ratatui::Frame<'_>) -> GraphicsFrame,
) -> Result<GraphicsFrame>
where
    B: Backend,
    std::io::Error: From<B::Error>,
{
    let mut graphics = GraphicsFrame::default();
    terminal.draw(|frame| graphics = render(frame))?;
    Ok(graphics)
}

fn render_frame<N, M>(frame: &mut ratatui::Frame<'_>, root: &N, area: Rect) -> GraphicsFrame
where
    N: TuiNode<M>,
{
    let area = area.intersection(frame.area());
    frame
        .buffer_mut()
        .set_style(area, Style::default().bg(theme().background_bg()));
    let mut ctx = RenderCtx::new();
    root.render(frame, area, &mut ctx);
    ctx.flush(frame);
    restore_theme_background(frame, area);
    ctx.take_graphics_frame()
}

fn render_frame_with_toasts_and_fade<N, M>(
    frame: &mut ratatui::Frame<'_>,
    root: &N,
    toasts: &ToastRack,
    area: Rect,
    fade_amount: f64,
) -> GraphicsFrame
where
    N: TuiNode<M>,
{
    let area = area.intersection(frame.area());
    let graphics = render_frame(frame, root, area);
    toasts.render(frame, area);
    restore_theme_background(frame, area);
    if fade_amount > 0.0 {
        fade_buffer(frame, area, fade_amount);
    }
    graphics
}

fn emit_direct_kitty(backend: &mut impl Write, commands: Vec<DirectKittyCommand>) -> Result<()> {
    for command in commands {
        match command {
            DirectKittyCommand::DeletePlacement(id) => write!(
                backend,
                "\x1b_Ga=d,d=i,i={},p={},q=2\x1b\\",
                id.image_id, id.placement_id
            )?,
            DirectKittyCommand::DeleteImage(image_id) => {
                write!(backend, "\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\")?
            }
            DirectKittyCommand::Transmit { payload, .. } => {
                backend.write_all(payload.as_bytes())?
            }
            DirectKittyCommand::Place {
                id,
                area,
                source_rect,
                z_index,
            } => write!(
                backend,
                "\x1b7\x1b[{};{}H\x1b_Ga=p,i={},p={},x={},y={},w={},h={},c={},r={},z={},C=1,q=2\x1b\\\x1b8",
                area.y.saturating_add(1),
                area.x.saturating_add(1),
                id.image_id,
                id.placement_id,
                source_rect.x,
                source_rect.y,
                source_rect.width,
                source_rect.height,
                area.width,
                area.height,
                z_index,
            )?,
        }
    }
    backend.flush()?;
    Ok(())
}

fn restore_theme_background(frame: &mut ratatui::Frame<'_>, area: Rect) {
    let background = theme().background_bg();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            if cell.bg == Color::Reset {
                cell.set_bg(background);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        Frame, Terminal,
        backend::TestBackend,
        layout::Rect,
        style::{Color, Style},
    };

    use super::*;
    use crate::{
        Calendar, EventCtx, EventOutcome, LayoutCtx, LayoutResult, OverlayLayer, Panel, TuiEvent,
    };

    struct EmptyNode;

    impl TuiNode<()> for EmptyNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut RenderCtx<'_>) {}
    }

    #[test]
    fn runtime_paints_the_theme_background_across_the_app_area() {
        let area = Rect::new(1, 1, 3, 2);
        let mut terminal = Terminal::new(TestBackend::new(5, 4)).expect("terminal should build");
        let expected = theme().background_bg();

        terminal
            .draw(|frame| {
                render_frame(frame, &EmptyNode, area);
            })
            .expect("frame should render");

        let buffer = terminal.backend().buffer();
        assert_eq!(buffer.cell((1, 1)).unwrap().bg, expected);
        assert_eq!(buffer.cell((3, 2)).unwrap().bg, expected);
        assert_eq!(buffer.cell((0, 0)).unwrap().bg, Color::Reset);
    }

    #[test]
    fn runtime_clips_a_stale_render_area_to_the_frame() {
        let mut terminal = Terminal::new(TestBackend::new(5, 4)).expect("terminal should build");

        terminal
            .draw(|frame| {
                render_frame_with_toasts_and_fade(
                    frame,
                    &EmptyNode,
                    &ToastRack::new(),
                    Rect::new(0, 0, 6, 4),
                    0.5,
                );
            })
            .expect("frame should render");

        assert_eq!(
            terminal.backend().buffer().cell((4, 3)).unwrap().bg,
            theme().background_bg()
        );
    }

    #[test]
    fn panels_preserve_the_theme_background_inside_their_borders() {
        let area = Rect::new(0, 0, 8, 3);
        let panel = Panel::<()>::new();
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height))
            .expect("terminal should build");

        terminal
            .draw(|frame| {
                render_frame(frame, &panel, frame.area());
            })
            .expect("frame should render");

        assert_eq!(
            terminal.backend().buffer().cell((1, 1)).unwrap().bg,
            theme().background_bg()
        );
    }

    #[test]
    fn calendars_preserve_the_theme_background_inside_their_panels() {
        let mut calendar = Calendar::<(), (), ()>::new(
            Vec::new(),
            |_| (),
            |_| unreachable!("calendar spans are not needed without entries"),
            |_| String::new(),
        );
        let area = Rect::new(0, 0, 20, 12);
        calendar.layout(area, &mut LayoutCtx::new());
        let mut terminal = Terminal::new(TestBackend::new(area.width, area.height))
            .expect("terminal should build");

        terminal
            .draw(|frame| {
                render_frame(frame, &calendar, frame.area());
            })
            .expect("frame should render");

        assert_eq!(
            terminal.backend().buffer().cell((1, 1)).unwrap().bg,
            theme().background_bg()
        );
    }

    struct PortalColorNode;

    impl TuiNode<()> for PortalColorNode {
        fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
            LayoutResult::new(area)
        }

        fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
            frame.buffer_mut().set_style(
                area,
                Style::default()
                    .fg(Color::Rgb(200, 200, 200))
                    .bg(Color::Rgb(10, 20, 30)),
            );
            ctx.push_portal(OverlayLayer::Modal, 0, area, |frame, area| {
                frame.buffer_mut().set_style(
                    area,
                    Style::default()
                        .fg(Color::Rgb(255, 100, 50))
                        .bg(Color::Rgb(40, 60, 80)),
                );
            });
        }

        fn event(&mut self, _event: &TuiEvent, _ctx: &mut EventCtx<()>) -> EventOutcome {
            EventOutcome::Ignored
        }
    }

    #[test]
    fn runtime_fade_applies_after_portals() {
        let area = Rect::new(0, 0, 10, 5);
        let mut terminal = Terminal::new(TestBackend::new(10, 5)).expect("terminal should build");

        terminal
            .draw(|frame| {
                render_frame_with_toasts_and_fade(
                    frame,
                    &PortalColorNode,
                    &ToastRack::new(),
                    area,
                    0.5,
                );
            })
            .expect("frame should render");

        let cell = terminal.backend().buffer().cell((0, 0)).unwrap();
        assert_ne!(cell.fg, Color::Rgb(255, 100, 50));
        assert_ne!(cell.bg, Color::Rgb(40, 60, 80));
    }

    #[test]
    fn empty_graphics_frames_clean_up_once_and_visible_graphics_retransmit() {
        let mut graphics = DirectKittyGraphics::default();
        let image = kitty_intent(9, Rect::new(1, 1, 2, 1));
        let _ = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&image)],
            intents: vec![image.clone()],
        });

        let cleanup = graphics.reconcile(GraphicsFrame::default());
        let suppressed = graphics.reconcile(GraphicsFrame::default());
        let final_frame = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&image)],
            intents: vec![image],
        });

        assert_eq!(command_kinds(&cleanup), vec![("delete-image", 9)]);
        assert!(suppressed.is_empty());
        assert_eq!(
            command_kinds(&final_frame),
            vec![("transmit", 9), ("place", 9)]
        );
        let mut output = Vec::new();
        emit_direct_kitty(&mut output, cleanup).expect("full-image cleanup should serialize");
        assert_eq!(
            String::from_utf8(output).expect("Kitty commands are UTF-8"),
            "\x1b_Ga=d,d=I,i=9,q=2\x1b\\"
        );
    }

    #[test]
    fn initially_hidden_resident_transmits_when_it_becomes_visible() {
        let mut graphics = DirectKittyGraphics::default();
        let visible = kitty_intent(9, Rect::new(1, 1, 4, 4));

        let hidden = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&visible)],
            intents: Vec::new(),
        });
        let appeared = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&visible)],
            intents: vec![visible],
        });

        assert!(hidden.is_empty());
        assert_eq!(
            command_kinds(&appeared),
            vec![("transmit", 9), ("place", 9)]
        );
    }

    #[test]
    fn resident_kitty_image_survives_hidden_placement_until_node_removal() {
        let mut graphics = DirectKittyGraphics::default();
        let visible = kitty_intent(9, Rect::new(1, 1, 4, 4));
        let mut partial = visible.clone();
        partial.area = Rect::new(1, 1, 4, 2);
        partial.source_rect.height = 40;

        let first = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&visible)],
            intents: vec![visible.clone()],
        });
        let clipped = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&partial)],
            intents: vec![partial],
        });
        let hidden = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&visible)],
            intents: Vec::new(),
        });
        let reappeared = graphics.reconcile(GraphicsFrame {
            residents: vec![kitty_resident(&visible)],
            intents: vec![visible],
        });
        let removed = graphics.reconcile(GraphicsFrame::default());

        assert_eq!(command_kinds(&first), vec![("transmit", 9), ("place", 9)]);
        assert_eq!(command_kinds(&clipped), vec![("place", 9)]);
        assert_eq!(command_kinds(&hidden), vec![("delete-placement", 9)]);
        assert_eq!(command_kinds(&reappeared), vec![("place", 9)]);
        assert_eq!(command_kinds(&removed), vec![("delete-image", 9)]);
    }

    #[test]
    fn kitty_placement_serializes_source_crop_and_destination_cells() {
        let mut output = Vec::new();

        emit_direct_kitty(
            &mut output,
            vec![DirectKittyCommand::Place {
                id: DirectKittyPlacementId {
                    image_id: 7,
                    placement_id: 3,
                },
                area: Rect::new(4, 5, 6, 2),
                source_rect: DirectKittySourceRect {
                    x: 11,
                    y: 13,
                    width: 120,
                    height: 40,
                },
                z_index: -9,
            }],
        )
        .expect("cropped placement should serialize");

        assert_eq!(
            String::from_utf8(output).expect("Kitty commands are UTF-8"),
            "\x1b7\x1b[6;5H\x1b_Ga=p,i=7,p=3,x=11,y=13,w=120,h=40,c=6,r=2,z=-9,C=1,q=2\x1b\\\x1b8"
        );
    }

    fn kitty_intent(image_id: u32, area: Rect) -> DirectKittyIntent {
        kitty_intent_at(image_id, area, GraphicsLevel::base())
    }

    fn kitty_intent_at(image_id: u32, area: Rect, level: GraphicsLevel) -> DirectKittyIntent {
        DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id,
                placement_id: 1,
            },
            area,
            source_rect: DirectKittySourceRect {
                x: 0,
                y: 0,
                width: u32::from(area.width) * 10,
                height: u32::from(area.height) * 20,
            },
            generation: 0,
            payload: Arc::from("payload"),
            level,
            z_index: level.kitty_image_z_index(),
        }
    }

    fn kitty_resident(intent: &DirectKittyIntent) -> DirectKittyResident {
        DirectKittyResident {
            image_id: intent.id.image_id,
            payload: Arc::clone(&intent.payload),
        }
    }

    fn command_kinds(commands: &[DirectKittyCommand]) -> Vec<(&'static str, u32)> {
        commands
            .iter()
            .map(|command| match command {
                DirectKittyCommand::DeletePlacement(id) => ("delete-placement", id.image_id),
                DirectKittyCommand::DeleteImage(image_id) => ("delete-image", *image_id),
                DirectKittyCommand::Transmit { image_id, .. } => ("transmit", *image_id),
                DirectKittyCommand::Place { id, .. } => ("place", id.image_id),
            })
            .collect()
    }
}
