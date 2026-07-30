use std::hash::Hash;
use std::time::Duration;

use ratatui::{Frame, layout::Rect};

use super::{
    ActivationMode, CheckState, ListControl, ListControlEvent, SelectionGlyphs, SelectionMode,
    SelectionPropagation, SelectionTrigger, TreeAdapter,
};
use crate::{
    AnimationSettings, EventCtx, EventOutcome, EventRoute, FocusCtx, FocusTarget, LayoutCtx,
    LayoutProposal, LayoutResult, LayoutSizeHint, LifecycleCtx, RenderCtx, TickResult, TuiEvent,
    TuiNode,
};

pub struct Checklist<T, Id, M = ()> {
    control: ListControl<T, Id, M>,
}

impl<T, Id, M: 'static> Checklist<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    pub fn new(
        rows: impl IntoIterator<Item = T>,
        row_id: impl Fn(&T) -> Id + 'static,
        label: impl Fn(&T) -> String + 'static,
        creator: impl FnMut(String, &[T]) -> T + 'static,
    ) -> Self {
        Self::from_list_control(ListControl::list(rows, row_id, label, creator))
    }

    pub fn from_list_control(control: ListControl<T, Id, M>) -> Self {
        Self {
            control: control
                .activation_mode(ActivationMode::Manual)
                .selection_mode(SelectionMode::Multi)
                .selection_trigger(SelectionTrigger::OnActivate)
                .selection_glyphs(SelectionGlyphs::NERD_FONT),
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.control = self.control.title(title);
        self
    }

    pub fn hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.control = self.control.hotkey(hotkey);
        self
    }

    pub fn empty_message(mut self, message: impl Into<String>) -> Self {
        self.control = self.control.empty_message(message);
        self
    }

    pub fn max_rows(mut self, max_rows: usize) -> Self {
        self.control = self.control.max_rows(max_rows);
        self
    }

    pub fn panel_visible(mut self, visible: bool) -> Self {
        self.control = self.control.panel_visible(visible);
        self
    }

    pub fn tree(mut self, tree: TreeAdapter<T, Id>) -> Self {
        self.control = self.control.tree(tree);
        self
    }

    pub fn expanded(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.control = self.control.expanded(ids);
        self
    }

    pub fn checked(mut self, ids: impl IntoIterator<Item = Id>) -> Self {
        self.control = self.control.selected(ids);
        self
    }

    pub fn cascade_descendants(mut self, enabled: bool) -> Self {
        let propagation = if enabled {
            SelectionPropagation::CascadeDescendants
        } else {
            SelectionPropagation::None
        };
        self.control = self.control.selection_propagation(propagation);
        self
    }

    pub fn checked_ids(&self) -> Vec<Id> {
        self.control.data_view().selected_ids()
    }

    pub fn check_state(&self, id: &Id) -> CheckState {
        self.control.data_view().check_state(id)
    }

    pub fn items(&self) -> &[T] {
        self.control.items()
    }

    pub fn list_control(&self) -> &ListControl<T, Id, M> {
        &self.control
    }

    pub fn list_control_mut(&mut self) -> &mut ListControl<T, Id, M> {
        &mut self.control
    }

    pub fn take_events(&mut self) -> Vec<ListControlEvent<Id>> {
        self.control.take_events()
    }
}

impl<T, Id, M: 'static> TuiNode<M> for Checklist<T, Id, M>
where
    T: 'static,
    Id: Clone + Eq + Hash,
{
    fn measure(&self, proposal: LayoutProposal) -> LayoutSizeHint {
        self.control.measure(proposal)
    }

    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.control.layout(area, ctx)
    }

    fn render<'a>(&'a self, frame: &mut Frame, area: Rect, ctx: &mut RenderCtx<'a>) {
        self.control.render(frame, area, ctx);
    }

    fn event(&mut self, event: &TuiEvent, ctx: &mut EventCtx<M>) -> EventOutcome {
        self.control.event(event, ctx)
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<M>,
    ) -> EventOutcome {
        self.control.dispatch_event(route, event, ctx)
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.control.tick(dt, settings)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<M>) {
        self.control.dispatch_focus(target, focused, ctx);
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<M>) {
        self.control.destroy(ctx);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::layout::Rect;

    use super::*;
    use crate::{Key, TreeAdapter};

    #[derive(Clone)]
    struct Item {
        id: usize,
        parent: Option<usize>,
        label: String,
    }

    fn checklist() -> Checklist<Item, usize> {
        Checklist::new(
            [
                Item {
                    id: 1,
                    parent: None,
                    label: "Release".into(),
                },
                Item {
                    id: 2,
                    parent: Some(1),
                    label: "Tests".into(),
                },
            ],
            |item| item.id,
            |item| item.label.clone(),
            |label, rows| Item {
                id: rows.len() + 1,
                parent: None,
                label,
            },
        )
        .tree(TreeAdapter::parent_id(|item: &Item| item.parent))
        .expanded([1])
        .cascade_descendants(true)
    }

    #[test]
    fn enter_checks_highlighted_item_and_emits_change() {
        let mut checklist = checklist();

        checklist
            .list_control_mut()
            .data_view_mut()
            .on_key(Key::Enter, Rect::new(0, 0, 30, 5));

        assert_eq!(checklist.checked_ids(), vec![1, 2]);
        assert_eq!(
            checklist.take_events(),
            vec![ListControlEvent::CheckedChanged {
                checked: vec![1, 2],
                added: vec![1, 2],
                removed: vec![],
            }]
        );
    }

    #[test]
    fn space_only_toggles_tree_expansion() {
        let mut checklist = checklist();

        checklist
            .list_control_mut()
            .data_view_mut()
            .on_key(Key::Char(' '), Rect::new(0, 0, 30, 5));

        assert!(checklist.checked_ids().is_empty());
        assert!(checklist.take_events().is_empty());
    }
}
