use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use base64::Engine;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::Rect,
    style::{Color, Style},
};

use crate::{OverlayLayer, RenderCtx, ToastRack, TuiNode, fade_buffer, theme};

use super::Result;

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
    pub generation: u64,
    pub payload: Arc<str>,
    pub level: GraphicsLevel,
    pub z_index: i32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OpaqueKittyMaskIntent {
    pub owner: u64,
    pub area: Rect,
    pub color: Color,
    pub level: GraphicsLevel,
}

#[derive(Debug, Default)]
pub(crate) struct GraphicsFrame {
    pub intents: Vec<DirectKittyIntent>,
    pub opaque_masks: Vec<OpaqueKittyMaskIntent>,
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
        z_index: i32,
    },
}

#[derive(Debug, Default)]
struct DirectKittyGraphics {
    active: BTreeMap<DirectKittyPlacementId, DirectKittyIntent>,
    mask_image_ids: BTreeMap<u64, u32>,
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

    fn kitty_mask_z_index(self) -> i32 {
        self.kitty_image_z_index() - 1
    }
}

impl DirectKittyGraphics {
    fn reconcile(&mut self, frame: GraphicsFrame) -> Vec<DirectKittyCommand> {
        let mut desired = BTreeMap::new();
        let mut display_order = Vec::new();
        for intent in frame.intents {
            if desired.insert(intent.id, intent.clone()).is_none() {
                display_order.push(intent.id);
            }
        }
        let mask_owners = frame
            .opaque_masks
            .iter()
            .map(|mask| mask.owner)
            .collect::<BTreeSet<_>>();
        self.mask_image_ids
            .retain(|owner, _| mask_owners.contains(owner));
        for mask in frame.opaque_masks {
            if mask.area.is_empty() {
                continue;
            }
            let image_id = *self
                .mask_image_ids
                .entry(mask.owner)
                .or_insert_with(next_direct_kitty_image_id);
            let id = DirectKittyPlacementId {
                image_id,
                placement_id: 1,
            };
            let intent = opaque_mask_intent(id, mask);
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

        let desired_image_ids = desired
            .keys()
            .map(|id| id.image_id)
            .collect::<BTreeSet<_>>();
        let changed_image_ids = desired
            .values()
            .filter(|intent| {
                self.transmitted
                    .get(&intent.id.image_id)
                    .is_some_and(|payload| payload != &intent.payload)
            })
            .map(|intent| intent.id.image_id)
            .collect::<BTreeSet<_>>();

        let mut commands = Vec::new();
        let mut deleted_images = BTreeSet::new();
        for (id, active) in &self.active {
            if desired.get(id) == Some(active) {
                continue;
            }
            if changed_image_ids.contains(&id.image_id) {
                if deleted_images.insert(id.image_id) {
                    commands.push(DirectKittyCommand::DeleteImage(id.image_id));
                    self.transmitted.remove(&id.image_id);
                }
            } else if !desired.contains_key(id) && desired_image_ids.contains(&id.image_id) {
                commands.push(DirectKittyCommand::DeletePlacement(*id));
            } else if !desired_image_ids.contains(&id.image_id)
                && deleted_images.insert(id.image_id)
            {
                commands.push(DirectKittyCommand::DeleteImage(id.image_id));
                self.transmitted.remove(&id.image_id);
            }
        }
        for id in display_order {
            let intent = desired
                .get(&id)
                .expect("display order only contains registered Kitty placements");
            if self.transmitted.get(&id.image_id) != Some(&intent.payload) {
                if self.transmitted.contains_key(&id.image_id) && deleted_images.insert(id.image_id)
                {
                    commands.push(DirectKittyCommand::DeleteImage(id.image_id));
                }
                commands.push(DirectKittyCommand::Transmit {
                    image_id: id.image_id,
                    payload: Arc::clone(&intent.payload),
                });
                self.transmitted
                    .insert(id.image_id, Arc::clone(&intent.payload));
            }
            if self.active.get(&id) != Some(intent) || changed_image_ids.contains(&id.image_id) {
                commands.push(DirectKittyCommand::Place {
                    id,
                    area: intent.area,
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
        self.mask_image_ids.clear();
        self.transmitted.clear();
        commands
    }
}

impl PartialEq for DirectKittyIntent {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.area == other.area
            && self.generation == other.generation
            && self.z_index == other.z_index
    }
}

fn opaque_mask_intent(
    id: DirectKittyPlacementId,
    mask: OpaqueKittyMaskIntent,
) -> DirectKittyIntent {
    let (red, green, blue) = color_to_rgb(mask.color);
    DirectKittyIntent {
        id,
        area: mask.area,
        generation: u64::from(red) << 16 | u64::from(green) << 8 | u64::from(blue),
        payload: opaque_mask_payload(id.image_id, (red, green, blue)),
        level: mask.level,
        z_index: mask.level.kitty_mask_z_index(),
    }
}

fn opaque_mask_payload(image_id: u32, color: (u8, u8, u8)) -> Arc<str> {
    let image =
        DynamicImage::ImageRgb8(RgbImage::from_pixel(1, 1, Rgb([color.0, color.1, color.2])));
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("one-pixel RGB mask encodes as PNG");
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    Arc::from(format!(
        "\x1b_Ga=t,t=d,f=100,i={image_id},q=2,m=0;{encoded}\x1b\\",
    ))
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Reset | Color::Black => (0, 0, 0),
        Color::Red => (128, 0, 0),
        Color::Green => (0, 128, 0),
        Color::Yellow => (128, 128, 0),
        Color::Blue => (0, 0, 128),
        Color::Magenta => (128, 0, 128),
        Color::Cyan => (0, 128, 128),
        Color::Gray => (192, 192, 192),
        Color::DarkGray => (128, 128, 128),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (0, 0, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Rgb(red, green, blue) => (red, green, blue),
        Color::Indexed(index) => indexed_color_to_rgb(index),
    }
}

fn indexed_color_to_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 255),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    match index {
        0..=15 => ANSI[index as usize],
        16..=231 => {
            const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
            let offset = index - 16;
            (
                LEVELS[(offset / 36) as usize],
                LEVELS[((offset % 36) / 6) as usize],
                LEVELS[(offset % 6) as usize],
            )
        }
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            (level, level, level)
        }
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
        terminal
            .draw(|frame| {
                render_frame(frame, root, area);
            })
            .map_err(Into::into)?;
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
        terminal
            .draw(|frame| {
                render_frame_with_toasts_and_fade(frame, root, toasts, area, 0.0);
            })
            .map_err(Into::into)?;
        Ok(())
    }

    pub(crate) fn render_with_toasts_and_fade<B, N, M>(
        &mut self,
        terminal: &mut Terminal<B>,
        root: &N,
        toasts: &ToastRack,
        area: Rect,
        fade_amount: f64,
    ) -> Result<()>
    where
        B: Backend,
        N: TuiNode<M>,
        std::io::Error: From<B::Error>,
    {
        terminal
            .draw(|frame| {
                render_frame_with_toasts_and_fade(frame, root, toasts, area, fade_amount);
            })
            .map_err(Into::into)?;
        Ok(())
    }

    pub(crate) fn render_with_toasts_and_fade_to_crossterm<W, N, M>(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<W>>,
        root: &N,
        toasts: &ToastRack,
        area: Rect,
        fade_amount: f64,
    ) -> Result<()>
    where
        W: Write,
        N: TuiNode<M>,
    {
        let graphics = draw_frame(terminal, |frame| {
            render_frame_with_toasts_and_fade(frame, root, toasts, area, fade_amount)
        })?;
        emit_direct_kitty(
            terminal.backend_mut(),
            self.direct_kitty.reconcile(graphics),
        )
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
    terminal
        .draw(|frame| graphics = render(frame))
        .map_err(Into::into)?;
    Ok(graphics)
}

fn render_frame<N, M>(frame: &mut ratatui::Frame<'_>, root: &N, area: Rect) -> GraphicsFrame
where
    N: TuiNode<M>,
{
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
            DirectKittyCommand::Place { id, area, z_index } => write!(
                backend,
                "\x1b7\x1b[{};{}H\x1b_Ga=p,i={},p={},c={},r={},z={},C=1,q=2\x1b\\\x1b8",
                area.y.saturating_add(1),
                area.x.saturating_add(1),
                id.image_id,
                id.placement_id,
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
    fn opening_a_dialog_displays_a_mask_without_deleting_the_base_preview() {
        let mut graphics = DirectKittyGraphics::default();
        let image = kitty_intent(1001, Rect::new(2, 2, 3, 2));

        let initial = graphics.reconcile(GraphicsFrame {
            intents: vec![image.clone()],
            opaque_masks: Vec::new(),
        });
        assert_eq!(
            command_kinds(&initial),
            vec![("transmit", 1001), ("place", 1001)]
        );

        let mask = modal_mask(50, Rect::new(0, 0, 10, 8));
        let expected_mask_z = mask.level.kitty_mask_z_index();
        let opened = graphics.reconcile(GraphicsFrame {
            intents: vec![image.clone()],
            opaque_masks: vec![mask],
        });
        assert_eq!(opened.len(), 2);
        let mask_id = placed_image_ids(&opened)[0];
        assert_ne!(mask_id, 1001);
        assert_eq!(placed_z_indexes(&opened), vec![(mask_id, expected_mask_z)]);
        assert!(!command_kinds(&opened).contains(&("delete-image", 1001)));
        assert!(!command_kinds(&opened).contains(&("delete-placement", 1001)));
        let mut output = Vec::new();
        emit_direct_kitty(&mut output, opened).expect("mask display serializes");
        let output = String::from_utf8(output).expect("Kitty commands are UTF-8");
        assert!(output.starts_with("\x1b_Ga=t,t=d,f=100,"));
        assert!(output.contains(&format!(
            "\x1b7\x1b[1;1H\x1b_Ga=p,i={mask_id},p=1,c=10,r=8,z={expected_mask_z},C=1,q=2"
        )));
        assert!(output.ends_with("\x1b\\\x1b8"));

        let closed = graphics.reconcile(GraphicsFrame {
            intents: vec![image],
            opaque_masks: Vec::new(),
        });
        assert_eq!(command_kinds(&closed), vec![("delete-image", mask_id)]);
    }

    #[test]
    fn dialog_kitty_image_is_placed_above_the_mask_and_base_preview() {
        let mut graphics = DirectKittyGraphics::default();
        let base = kitty_intent(1002, Rect::new(2, 2, 3, 2));
        let _ = graphics.reconcile(GraphicsFrame {
            intents: vec![base.clone()],
            opaque_masks: Vec::new(),
        });
        let dialog = dialog_kitty_intent(1003, Rect::new(2, 2, 3, 2), 51);
        let mask = modal_mask(51, Rect::new(0, 0, 10, 8));

        let opened = graphics.reconcile(GraphicsFrame {
            intents: vec![base.clone(), dialog.clone()],
            opaque_masks: vec![mask],
        });

        let mask_id = placed_image_ids(&opened)
            .into_iter()
            .find(|image_id| *image_id != 1003)
            .expect("dialog opening places its mask");
        let z_indexes = placed_z_indexes(&opened);
        let mask_z = z_indexes
            .iter()
            .find_map(|(image_id, z)| (*image_id == mask_id).then_some(*z))
            .expect("mask has a physical z-index");
        let dialog_z = z_indexes
            .iter()
            .find_map(|(image_id, z)| (*image_id == 1003).then_some(*z))
            .expect("dialog image has a physical z-index");
        assert!(BASE_DIRECT_KITTY_Z_INDEX < mask_z);
        assert!(mask_z < dialog_z);
        assert_eq!(placed_areas_for_image(&opened, mask_id), vec![mask.area]);
        assert_eq!(transmitted_image_ids(&opened), vec![mask_id, 1003]);

        let unchanged = graphics.reconcile(GraphicsFrame {
            intents: vec![base.clone(), dialog.clone()],
            opaque_masks: vec![mask],
        });
        assert!(unchanged.is_empty());

        let moved_dialog = dialog_kitty_intent(1003, Rect::new(0, 2, 10, 2), 51);
        let moved = graphics.reconcile(GraphicsFrame {
            intents: vec![base.clone(), moved_dialog],
            opaque_masks: vec![mask],
        });
        assert_eq!(command_kinds(&moved), vec![("place", 1003)]);

        let closed = graphics.reconcile(GraphicsFrame {
            intents: vec![base],
            opaque_masks: Vec::new(),
        });
        assert!(!command_image_ids(&closed).contains(&1002));
        assert!(command_kinds(&closed).contains(&("delete-image", mask_id)));
        assert!(command_kinds(&closed).contains(&("delete-image", 1003)));
    }

    #[test]
    fn nested_modal_masks_clean_up_by_owner() {
        let mut graphics = DirectKittyGraphics::default();
        let opened = graphics.reconcile(GraphicsFrame {
            intents: Vec::new(),
            opaque_masks: vec![
                modal_mask(60, Rect::new(0, 0, 10, 8)),
                modal_mask(61, Rect::new(2, 2, 6, 4)),
            ],
        });
        let mask_ids = opened
            .iter()
            .filter_map(|command| match command {
                DirectKittyCommand::Place { id, .. } => Some(id.image_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(mask_ids.len(), 2);
        let z_indexes = placed_z_indexes(&opened);
        assert!(z_indexes[0].1 < z_indexes[1].1);

        let outer_closed = graphics.reconcile(GraphicsFrame {
            intents: Vec::new(),
            opaque_masks: vec![modal_mask(61, Rect::new(2, 2, 6, 4))],
        });
        assert_eq!(
            command_kinds(&outer_closed),
            vec![("delete-image", mask_ids[0])]
        );

        let all_closed = graphics.reconcile(GraphicsFrame::default());
        assert_eq!(
            command_kinds(&all_closed),
            vec![("delete-image", mask_ids[1])]
        );
    }

    #[test]
    fn removing_a_kitty_placement_emits_targeted_cleanup() {
        let mut graphics = DirectKittyGraphics::default();
        let image = kitty_intent(9, Rect::new(1, 1, 2, 1));
        let _ = graphics.reconcile(GraphicsFrame {
            intents: vec![image],
            opaque_masks: Vec::new(),
        });

        let removed = graphics.reconcile(GraphicsFrame::default());

        assert_eq!(command_kinds(&removed), vec![("delete-image", 9)]);
        let mut output = Vec::new();
        emit_direct_kitty(&mut output, removed).expect("targeted cleanup should serialize");
        assert_eq!(
            String::from_utf8(output).expect("Kitty commands are UTF-8"),
            "\x1b_Ga=d,d=I,i=9,q=2\x1b\\"
        );
    }

    fn kitty_intent(image_id: u32, area: Rect) -> DirectKittyIntent {
        kitty_intent_at(image_id, area, GraphicsLevel::base())
    }

    fn dialog_kitty_intent(image_id: u32, area: Rect, order: u64) -> DirectKittyIntent {
        kitty_intent_at(
            image_id,
            area,
            GraphicsLevel::new(OverlayLayer::Modal, 0, order),
        )
    }

    fn kitty_intent_at(image_id: u32, area: Rect, level: GraphicsLevel) -> DirectKittyIntent {
        DirectKittyIntent {
            id: DirectKittyPlacementId {
                image_id,
                placement_id: 1,
            },
            area,
            generation: 0,
            payload: Arc::from("payload"),
            level,
            z_index: level.kitty_image_z_index(),
        }
    }

    fn modal_mask(owner: u64, area: Rect) -> OpaqueKittyMaskIntent {
        OpaqueKittyMaskIntent {
            owner,
            area,
            color: Color::Rgb(40, 60, 80),
            level: GraphicsLevel::new(OverlayLayer::Modal, 0, owner),
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

    fn placed_areas_for_image(commands: &[DirectKittyCommand], image_id: u32) -> Vec<Rect> {
        commands
            .iter()
            .filter_map(|command| match command {
                DirectKittyCommand::Place { id, area, .. } if id.image_id == image_id => {
                    Some(*area)
                }
                _ => None,
            })
            .collect()
    }

    fn placed_image_ids(commands: &[DirectKittyCommand]) -> Vec<u32> {
        commands
            .iter()
            .filter_map(|command| match command {
                DirectKittyCommand::Place { id, .. } => Some(id.image_id),
                _ => None,
            })
            .collect()
    }

    fn transmitted_image_ids(commands: &[DirectKittyCommand]) -> Vec<u32> {
        commands
            .iter()
            .filter_map(|command| match command {
                DirectKittyCommand::Transmit { image_id, .. } => Some(*image_id),
                _ => None,
            })
            .collect()
    }

    fn placed_z_indexes(commands: &[DirectKittyCommand]) -> Vec<(u32, i32)> {
        commands
            .iter()
            .filter_map(|command| match command {
                DirectKittyCommand::Place { id, z_index, .. } => Some((id.image_id, *z_index)),
                _ => None,
            })
            .collect()
    }

    fn command_image_ids(commands: &[DirectKittyCommand]) -> Vec<u32> {
        commands
            .iter()
            .map(|command| match command {
                DirectKittyCommand::DeletePlacement(id) | DirectKittyCommand::Place { id, .. } => {
                    id.image_id
                }
                DirectKittyCommand::DeleteImage(image_id)
                | DirectKittyCommand::Transmit { image_id, .. } => *image_id,
            })
            .collect()
    }
}
