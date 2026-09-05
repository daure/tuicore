use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Clear, Paragraph},
};
use tuicore::{
    Image, ImageProtocol, LayoutCtx, LayoutProposal, LayoutResult, LayoutSizeHint, RenderCtx,
    TickResult, TuiNode,
};

use crate::Msg;

const DOGGO_IMAGE: &str = "examples/assets/doggo.jpg";
const BASE64_IMAGE: &str = include_str!("../assets/31272.jpg.base64");
const URL_IMAGE: &str = "https://cdn.pixabay.com/photo/2015/11/16/14/43/cat-1045782_1280.jpg";
const MAX_IMAGE_SIZE: (u16, u16) = (48, 28);

pub(crate) struct ImageDemo {
    description: &'static str,
    image: Image,
}

impl ImageDemo {
    pub(crate) fn path() -> Self {
        Self::new(
            "Path · examples/assets/doggo.jpg",
            Image::from_path(DOGGO_IMAGE).expect("gallery dog image is available"),
        )
    }

    pub(crate) fn base64() -> Self {
        Self::new(
            "Base64 · generated from 31272.jpg",
            Image::from_base64(BASE64_IMAGE).expect("gallery base64 image is valid"),
        )
    }

    pub(crate) fn url() -> Self {
        Self::new(
            "URL · cdn.pixabay.com",
            Image::from_url(URL_IMAGE).expect("gallery URL image is available"),
        )
    }

    pub(crate) fn redraw(&mut self) {
        self.image.redraw();
    }

    pub(crate) fn tick(&mut self) -> TickResult {
        self.image.tick()
    }

    fn new(description: &'static str, image: Image) -> Self {
        Self {
            description,
            image: image.protocol(ImageProtocol::Kitty),
        }
    }
}

impl TuiNode<Msg> for ImageDemo {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        <Image as TuiNode<Msg>>::measure(&self.image, proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        <Image as TuiNode<Msg>>::layout(&mut self.image, image_area(area), ctx);
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [copy_area, _] = areas(area);
        frame.render_widget(
            Paragraph::new(format!(
                "{description} · Kitty graphics through Zellij / Ghostty\nUse Image::from_path, Image::from_base64, or Image::from_url.",
                description = self.description,
            )),
            copy_area,
        );
        let image_area = image_area(area);
        frame.render_widget(Clear, image_area);
        <Image as TuiNode<Msg>>::render(&self.image, frame, image_area, ctx);
    }

    fn init(&mut self, ctx: &mut tuicore::LifecycleCtx<Msg>) {
        self.image.init(ctx);
        self.image.preload(MAX_IMAGE_SIZE.0, MAX_IMAGE_SIZE.1);
    }
}

fn areas(area: Rect) -> [Rect; 2] {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Fill(1)])
        .areas(area)
}

fn image_area(area: Rect) -> Rect {
    let [_, image_area] = areas(area);
    Rect::new(
        image_area.x,
        image_area.y,
        image_area.width.min(MAX_IMAGE_SIZE.0),
        image_area.height.min(MAX_IMAGE_SIZE.1),
    )
}
