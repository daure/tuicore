use ratatui::{Terminal, backend::Backend, layout::Rect};

use crate::TuiNode;

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
            .draw(|frame| root.render(frame, area))
            .map_err(Into::into)?;
        Ok(())
    }
}
