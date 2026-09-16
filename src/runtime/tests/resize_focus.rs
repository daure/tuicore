use super::*;
use crate::{ChildKey, FocusCtx, FocusId, FocusTarget, LayoutCtx, LayoutResult, TreePath};

#[derive(Default)]
struct ResponsiveFocusRoot {
    wide: bool,
    focused: bool,
    pending_focus: Option<FocusRequest>,
    focus_log: Vec<(TreePath, bool)>,
}

fn details_path(wide: bool) -> TreePath {
    TreePath::from_keys([ChildKey::new(if wide { "desktop" } else { "mobile" })])
}

fn details_request(wide: bool) -> FocusRequest {
    FocusRequest::TargetAt {
        path: details_path(wide),
        id: FocusId::new("tabs"),
    }
}

impl TuiNode for ResponsiveFocusRoot {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        let wide = area.width >= 100;
        if wide != self.wide && self.focused {
            self.pending_focus = Some(details_request(wide));
        }
        self.wide = wide;
        ctx.push_slot(ChildKey::new("toolbar"), area, |ctx| {
            ctx.register_focusable(FocusId::new("add"), area, true);
        });
        ctx.push_slot(details_path(wide).first().unwrap().clone(), area, |ctx| {
            ctx.register_focusable(FocusId::new("tabs"), area, true);
        });
        LayoutResult::new(area)
    }

    fn render(&self, _: &mut ratatui::Frame, _: Rect, _: &mut crate::RenderCtx<'_>) {}

    fn take_pending_focus_request(&mut self) -> Option<FocusRequest> {
        self.pending_focus.take()
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, _: &mut FocusCtx<()>) {
        if target.id == FocusId::new("tabs") {
            self.focused = focused;
        }
        self.focus_log.push((target.path.clone(), focused));
    }
}

#[test]
fn resize_focus_moves_directly_to_the_new_path_before_any_tick() {
    let mut app =
        TreeApp::new(ResponsiveFocusRoot::default()).initial_focus(details_request(false));
    let mut flags = app.mount_root();
    let mut focus = FocusManager::new();
    let mut layout = LayoutEngine::new();
    let mut dispatcher = TreeDispatcher::new();

    for width in [96, 120, 96, 180] {
        let previous = focus.current().map(|target| target.path.clone());
        app.root.focus_log.clear();
        app.layout_root(
            &mut flags,
            &mut focus,
            &mut layout,
            &mut dispatcher,
            Rect::new(0, 0, width, 40),
        );
        assert!(
            app.root.focus_log.is_empty(),
            "layout must defer focus to the explicit request"
        );
        app.apply_pending_focus(&mut flags, &mut focus, &layout, &mut dispatcher, None);

        let expected = details_path(width >= 100);
        let mut expected_log = Vec::new();
        if let Some(previous) = previous {
            expected_log.push((previous, false));
        }
        expected_log.push((expected.clone(), true));
        assert_eq!(app.root.focus_log, expected_log);
        assert_eq!(focus.current().unwrap().path, expected);
        assert_eq!(app.root.take_pending_focus_request(), None);
    }
}

#[test]
fn explicit_focus_takes_priority_over_layout_restoration() {
    let mut app =
        TreeApp::new(ResponsiveFocusRoot::default()).initial_focus(details_request(false));
    let mut flags = app.mount_root();
    let mut focus = FocusManager::new();
    let mut layout = LayoutEngine::new();
    let mut dispatcher = TreeDispatcher::new();
    app.layout_root(
        &mut flags,
        &mut focus,
        &mut layout,
        &mut dispatcher,
        Rect::new(0, 0, 96, 40),
    );
    app.apply_pending_focus(&mut flags, &mut focus, &layout, &mut dispatcher, None);

    flags.focus_request = Some(FocusRequest::Target(FocusId::new("add")));
    app.layout_root(
        &mut flags,
        &mut focus,
        &mut layout,
        &mut dispatcher,
        Rect::new(0, 0, 120, 40),
    );
    app.apply_pending_focus(&mut flags, &mut focus, &layout, &mut dispatcher, None);

    assert_eq!(focus.current().unwrap().id, FocusId::new("add"));
    assert_eq!(app.root.take_pending_focus_request(), None);
}
