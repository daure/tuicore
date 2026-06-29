use ratatui::{Terminal, backend::Backend, layout::Rect};

use crate::{RenderCtx, ToastRack, TuiNode};

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
                let mut ctx = RenderCtx::new();
                root.render(frame, area, &mut ctx);
                ctx.flush(frame);
                toasts.render(frame, area);
            })
            .map_err(Into::into)?;
        Ok(())
    }
}
