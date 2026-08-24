use ratatui::{
    Terminal,
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
};

use crate::{RenderCtx, ToastRack, TuiNode, fade_buffer, theme};

use super::Result;

#[derive(Debug, Default)]
pub struct Renderer;

impl Renderer {
    pub fn new() -> Self {
        Self
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
}

fn render_frame<N, M>(frame: &mut ratatui::Frame<'_>, root: &N, area: Rect)
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
}

fn render_frame_with_toasts_and_fade<N, M>(
    frame: &mut ratatui::Frame<'_>,
    root: &N,
    toasts: &ToastRack,
    area: Rect,
    fade_amount: f64,
) where
    N: TuiNode<M>,
{
    render_frame(frame, root, area);
    toasts.render(frame, area);
    restore_theme_background(frame, area);
    if fade_amount > 0.0 {
        fade_buffer(frame, area, fade_amount);
    }
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
            .draw(|frame| render_frame(frame, &EmptyNode, area))
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
            .draw(|frame| render_frame(frame, &panel, frame.area()))
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
            .draw(|frame| render_frame(frame, &calendar, frame.area()))
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
}
