use std::sync::mpsc::{Receiver, TryRecvError, channel};

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Clear, Paragraph},
};
use tuicore::{
    AnimationSettings, ChildKey, Dropdown, DropdownSearchMode, EventCtx, EventOutcome, EventRoute,
    FocusCtx, FocusTarget, Image, ImageProtocol, Language, LayoutCtx, LayoutProposal, LayoutResult,
    LayoutSizeHint, MermaidRasterFitBox, MermaidRasterOptions, MermaidRenderer, RenderCtx,
    SyntaxHighlighter, TickResult, TuiEvent, TuiNode, theme,
};

use super::mermaid_examples::{EXAMPLES, MermaidExample};
use crate::Msg;

const MAX_IMAGE_SIZE: (u16, u16) = (56, 22);

enum GenerationEvent {
    Loading,
    Png(Vec<u8>),
    Error(String),
}

enum GenerationState {
    Idle,
    Generating,
    Loading,
    Ready,
    Error(String),
}

pub(crate) struct MermaidDemo {
    dropdown: Dropdown<MermaidExample, &'static str>,
    selected_id: &'static str,
    source: SyntaxHighlighter,
    image: Option<Image>,
    state: GenerationState,
    receiver: Option<Receiver<GenerationEvent>>,
}

impl MermaidDemo {
    pub(crate) fn new() -> Self {
        let first_id = EXAMPLES[0].id;
        let first_source = EXAMPLES[0].source;
        Self {
            dropdown: Dropdown::single(
                EXAMPLES.to_vec(),
                |row| row.id,
                |row| row.label.to_string(),
            )
            .label("Mermaid file")
            .search_mode(DropdownSearchMode::Contains)
            .selected([first_id]),
            selected_id: first_id,
            source: SyntaxHighlighter::new(first_source, Language::Markdown),
            image: None,
            state: GenerationState::Idle,
            receiver: None,
        }
    }

    pub(crate) fn generate(&mut self) {
        if self.receiver.is_some() {
            return;
        }

        let source = self.selected_example().source.to_owned();
        let theme = theme();
        let options = MermaidRasterOptions::default()
            .with_fit_to(MermaidRasterFitBox::contain(1120, 440))
            .with_scale(2.0);
        let (sender, receiver) = channel();
        self.image = None;
        self.state = GenerationState::Generating;
        self.receiver = Some(receiver);
        std::thread::spawn(move || {
            match MermaidRenderer::new().render_png_with_theme(source, &options, &theme) {
                Ok(png) => {
                    let _ = sender.send(GenerationEvent::Loading);
                    let _ = sender.send(GenerationEvent::Png(png));
                }
                Err(error) => {
                    let _ = sender.send(GenerationEvent::Error(error.to_string()));
                }
            }
        });
    }

    fn selected_example(&self) -> MermaidExample {
        let id = self.dropdown.selected_ids().first().copied();
        EXAMPLES
            .iter()
            .copied()
            .find(|example| Some(example.id) == id)
            .expect("Mermaid dropdown always selects a known example")
    }

    fn update_source(&mut self) {
        let example = self.selected_example();
        if example.id == self.selected_id {
            return;
        }
        self.selected_id = example.id;
        self.source.set_code(example.source);
        self.image = None;
        self.state = GenerationState::Idle;
        self.generate();
    }

    fn status(&self) -> Option<&str> {
        match &self.state {
            GenerationState::Generating => Some("Generating mermaid..."),
            GenerationState::Loading => Some("Loading file..."),
            GenerationState::Error(error) => Some(error),
            GenerationState::Idle | GenerationState::Ready => None,
        }
    }

    fn areas(area: Rect) -> [Rect; 3] {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Percentage(25),
                Constraint::Fill(1),
            ])
            .areas(area)
    }
}

impl TuiNode<Msg> for MermaidDemo {
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        LayoutSizeHint::content(64, 30).normalized(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let [dropdown, source, image_area] = Self::areas(area);
        ctx.push_slot(ChildKey::new("dropdown"), dropdown, |ctx| {
            TuiNode::<Msg>::layout(&mut self.dropdown, dropdown, ctx);
        });
        ctx.push_slot(ChildKey::new("source"), source, |ctx| {
            TuiNode::<Msg>::layout(&mut self.source, source, ctx);
        });
        if let Some(image) = &mut self.image {
            ctx.push_slot(ChildKey::new("image"), image_area, |ctx| {
                TuiNode::<Msg>::layout(image, image_area, ctx);
            });
        }
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        let [dropdown, source, image_area] = Self::areas(area);
        TuiNode::<Msg>::render(&self.dropdown, frame, dropdown, ctx);
        TuiNode::<Msg>::render(&self.source, frame, source, ctx);
        frame.render_widget(Clear, image_area);
        if let Some(rendered) = &self.image {
            TuiNode::<Msg>::render(rendered, frame, image_area, ctx);
        } else if let Some(status) = self.status() {
            frame.render_widget(Paragraph::new(status), image_area);
        }
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if let Some(path) = route.path.without_first_if(&ChildKey::new("dropdown")) {
            let outcome = TuiNode::<Msg>::dispatch_event(
                &mut self.dropdown,
                &EventRoute::new(path),
                event,
                ctx,
            );
            self.update_source();
            return outcome;
        }
        if let Some(path) = route.path.without_first_if(&ChildKey::new("source")) {
            return TuiNode::<Msg>::dispatch_event(
                &mut self.source,
                &EventRoute::new(path),
                event,
                ctx,
            );
        }
        EventOutcome::Ignored
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        if let Some(target) = target.for_child(&ChildKey::new("dropdown")) {
            TuiNode::<Msg>::dispatch_focus(&mut self.dropdown, &target, focused, ctx);
        }
        if let Some(target) = target.for_child(&ChildKey::new("source")) {
            TuiNode::<Msg>::dispatch_focus(&mut self.source, &target, focused, ctx);
        }
    }

    fn tick(&mut self, dt: std::time::Duration, settings: AnimationSettings) -> TickResult {
        let mut result = TuiNode::<Msg>::tick(&mut self.dropdown, dt, settings)
            .merge(TuiNode::<Msg>::tick(&mut self.source, dt, settings));
        if let Some(image) = &mut self.image {
            result = result.merge(image.tick());
        }

        let Some(receiver) = &self.receiver else {
            return result;
        };
        match receiver.try_recv() {
            Ok(GenerationEvent::Loading) => {
                self.state = GenerationState::Loading;
                result.merge(TickResult::CHANGED)
            }
            Ok(GenerationEvent::Png(png)) => {
                self.receiver = None;
                match Image::from_bytes(png) {
                    Ok(mut image) => {
                        image = image.protocol(ImageProtocol::Kitty);
                        image.preload(MAX_IMAGE_SIZE.0, MAX_IMAGE_SIZE.1);
                        self.image = Some(image);
                        self.state = GenerationState::Ready;
                    }
                    Err(error) => self.state = GenerationState::Error(error.to_string()),
                }
                result.merge(TickResult {
                    changed: true,
                    layout: true,
                    ..TickResult::IDLE
                })
            }
            Ok(GenerationEvent::Error(error)) => {
                self.receiver = None;
                self.state = GenerationState::Error(error);
                result.merge(TickResult::CHANGED)
            }
            Err(TryRecvError::Empty) => result.merge(TickResult::ACTIVE),
            Err(TryRecvError::Disconnected) => {
                self.receiver = None;
                self.state = GenerationState::Error("Mermaid generation stopped.".to_string());
                result.merge(TickResult::CHANGED)
            }
        }
    }

    fn init(&mut self, ctx: &mut tuicore::LifecycleCtx<Msg>) {
        self.dropdown.init(ctx);
        self.source.init(ctx);
    }

    fn mount(&mut self, ctx: &mut tuicore::LifecycleCtx<Msg>) {
        self.dropdown.mount(ctx);
        self.source.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut tuicore::LifecycleCtx<Msg>) {
        self.source.unmount(ctx);
        self.dropdown.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut tuicore::LifecycleCtx<Msg>) {
        self.source.destroy(ctx);
        self.dropdown.destroy(ctx);
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat};

    use super::*;

    #[test]
    fn completed_generation_requests_a_layout_for_the_new_image() {
        let mut demo = MermaidDemo::new();
        let (sender, receiver) = channel();
        let mut png = Vec::new();
        DynamicImage::new_rgba8(1, 1)
            .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
            .expect("test PNG encodes");
        sender.send(GenerationEvent::Png(png)).unwrap();
        demo.receiver = Some(receiver);

        let result = demo.tick(std::time::Duration::ZERO, AnimationSettings::default());

        assert!(result.layout);
    }

    #[test]
    fn every_gallery_example_renders_to_png() {
        let renderer = MermaidRenderer::new();

        for example in EXAMPLES {
            renderer
                .render_png(example.source)
                .unwrap_or_else(|error| panic!("{}: {error}", example.id));
        }
    }
}
