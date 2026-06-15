use std::error::Error;

use tuirealm::application::PollStrategy;
use tuirealm::props::{AttrValue, Attribute};
use tuirealm::ratatui::layout::{Constraint, Direction, Layout, Rect};
use tuirealm::terminal::{CrosstermTerminalAdapter, TerminalAdapter};

#[path = "gallery/component_list.rs"]
mod component_list;
#[path = "gallery/data_view_preview.rs"]
mod data_view_preview;
#[path = "gallery/panel_preview.rs"]
mod panel_preview;
#[path = "gallery/scroll_preview.rs"]
mod scroll_preview;
#[path = "gallery/shared.rs"]
mod shared;
#[path = "gallery/spinner_preview.rs"]
mod spinner_preview;
#[path = "gallery/tabs_preview.rs"]
mod tabs_preview;

use component_list::ComponentList;
use data_view_preview::{DataViewMode, DataViewPreview};
use panel_preview::PanelPreview;
use scroll_preview::AnimatedScrollPreview;
use shared::{ComponentKind, Id, Msg};
use spinner_preview::SpinnerPreview;
use tabs_preview::TabsPreview;
use tuicore::{FocusOutcome, FocusRouter, FocusWrap, TuicoreApp};
use tuirealm::event::NoUserEvent;

fn main() -> Result<(), Box<dyn Error>> {
    tuicore::init();
    let mut model = Model::new()?;

    while !model.quit {
        let frame_duration = tuicore::animation_settings().frame_duration();
        model.sync_event_areas()?;
        for msg in model.app.tick(PollStrategy::Once(frame_duration))? {
            model.update(msg)?;
        }

        if model.redraw {
            model.view()?;
            model.redraw = false;
        }
    }

    Ok(())
}

struct Model {
    app: TuicoreApp<Id, Msg, NoUserEvent>,
    terminal: CrosstermTerminalAdapter,
    selected: ComponentKind,
    focus: FocusRouter<GalleryFocus>,
    quit: bool,
    redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GalleryFocus {
    ComponentList,
    Preview,
}

const GALLERY_FOCUS_ORDER: [GalleryFocus; 2] = [GalleryFocus::ComponentList, GalleryFocus::Preview];

impl Model {
    fn new() -> Result<Self, Box<dyn Error>> {
        let mut app = TuicoreApp::new();
        app.mount(
            Id::ComponentList,
            ComponentList::new(ComponentKind::ALL.to_vec()),
        )?;
        app.mount(Id::Tabs, TabsPreview::new())?;
        app.mount(Id::Panel, PanelPreview::new())?;
        app.mount(Id::ScrollAnimated, AnimatedScrollPreview::new())?;
        app.mount(Id::Spinner, SpinnerPreview::new())?;
        app.mount(Id::DataViewList, DataViewPreview::new(DataViewMode::List))?;
        app.mount(Id::DataViewTable, DataViewPreview::new(DataViewMode::Table))?;
        app.mount(
            Id::DataViewListTree,
            DataViewPreview::new(DataViewMode::ListTree),
        )?;
        app.mount(
            Id::DataViewTableTree,
            DataViewPreview::new(DataViewMode::TableTree),
        )?;
        app.mount(
            Id::DataViewSingleSelect,
            DataViewPreview::new(DataViewMode::SingleSelect),
        )?;
        app.mount(
            Id::DataViewMultiSelect,
            DataViewPreview::new(DataViewMode::MultiSelect),
        )?;
        app.mount(
            Id::DataViewChecklistTree,
            DataViewPreview::new(DataViewMode::ChecklistTree),
        )?;
        app.mount(
            Id::DataViewActivateOnNavigate,
            DataViewPreview::new(DataViewMode::ActivateOnNavigate),
        )?;
        app.active(&Id::ComponentList)?;

        let mut terminal = CrosstermTerminalAdapter::new()?;
        terminal.enable_raw_mode()?;
        terminal.enter_alternate_screen()?;

        Ok(Self {
            app,
            terminal,
            selected: ComponentKind::Tabs,
            focus: FocusRouter::try_new(GALLERY_FOCUS_ORDER)?.with_wrap(FocusWrap::Wrap),
            quit: false,
            redraw: true,
        })
    }

    fn view(&mut self) -> Result<(), Box<dyn Error>> {
        self.terminal.draw(|frame| {
            let [left, right] = Self::layout(frame.area());

            self.app.view(&Id::ComponentList, frame, left);
            self.app.view(&self.selected.preview_id(), frame, right);
        })?;

        Ok(())
    }

    fn sync_event_areas(&mut self) -> Result<(), Box<dyn Error>> {
        let area = self.terminal.raw().size()?.into();
        let [list, preview] = Self::layout(area);
        let list_inner = tuicore::Panel::inner_area(list);
        self.app.attr(
            &Id::ComponentList,
            Attribute::Width,
            AttrValue::Size(list_inner.width),
        )?;
        self.app.attr(
            &Id::ComponentList,
            Attribute::Height,
            AttrValue::Size(list_inner.height),
        )?;
        for id in [
            Id::Tabs,
            Id::Panel,
            Id::ScrollAnimated,
            Id::Spinner,
            Id::DataViewList,
            Id::DataViewTable,
            Id::DataViewListTree,
            Id::DataViewTableTree,
            Id::DataViewSingleSelect,
            Id::DataViewMultiSelect,
            Id::DataViewChecklistTree,
            Id::DataViewActivateOnNavigate,
        ] {
            self.app
                .attr(&id, Attribute::Width, AttrValue::Size(preview.width))?;
            self.app
                .attr(&id, Attribute::Height, AttrValue::Size(preview.height))?;
        }
        Ok(())
    }

    fn layout(area: Rect) -> [Rect; 2] {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .areas(area)
    }

    fn update(&mut self, msg: Msg) -> Result<(), Box<dyn Error>> {
        self.redraw = true;
        match msg {
            Msg::Quit => self.quit = true,
            Msg::FocusNext => self.focus_next()?,
            Msg::FocusPrevious => self.focus_previous()?,
            Msg::FocusList => self.activate_focus(GalleryFocus::ComponentList)?,
            Msg::Selected(component) => self.selected = component,
            Msg::Redraw => {}
        }

        Ok(())
    }

    fn focus_next(&mut self) -> Result<(), Box<dyn Error>> {
        if let FocusOutcome::Moved { to, .. } = self.focus.focus_next() {
            self.activate_focus(to)?;
        }
        Ok(())
    }

    fn focus_previous(&mut self) -> Result<(), Box<dyn Error>> {
        if let FocusOutcome::Moved { to, .. } = self.focus.focus_previous() {
            self.activate_focus(to)?;
        }
        Ok(())
    }

    fn activate_focus(&mut self, focus: GalleryFocus) -> Result<(), Box<dyn Error>> {
        let _ = self.focus.focus(&focus);
        match focus {
            GalleryFocus::ComponentList => self.app.active(&Id::ComponentList)?,
            GalleryFocus::Preview => self.app.active(&self.selected.preview_id())?,
        }
        Ok(())
    }
}

impl Drop for Model {
    fn drop(&mut self) {
        let _ = self.terminal.leave_alternate_screen();
        let _ = self.terminal.disable_raw_mode();
    }
}
