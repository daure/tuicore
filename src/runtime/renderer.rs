use ratatui::{Terminal, backend::Backend, layout::Rect};

use crate::{RenderCtx, ToastRack, TuiNode, fade_buffer};

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
                let mut ctx = RenderCtx::new();
                root.render(frame, area, &mut ctx);
                ctx.flush(frame);
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

fn render_frame_with_toasts_and_fade<N, M>(
    frame: &mut ratatui::Frame<'_>,
    root: &N,
    toasts: &ToastRack,
    area: Rect,
    fade_amount: f64,
) where
    N: TuiNode<M>,
{
    let mut ctx = RenderCtx::new();
    root.render(frame, area, &mut ctx);
    ctx.flush(frame);
    toasts.render(frame, area);
    if fade_amount > 0.0 {
        fade_buffer(frame, area, fade_amount);
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
    use crate::{EventCtx, EventOutcome, LayoutCtx, LayoutResult, OverlayLayer, TuiEvent};

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
