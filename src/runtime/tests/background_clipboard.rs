use super::*;
use std::{cell::RefCell, rc::Rc};

struct CopyRoot(Option<String>);

impl TuiNode for CopyRoot {
    fn layout(&mut self, area: Rect, _: &mut crate::LayoutCtx) -> crate::LayoutResult {
        crate::LayoutResult::new(area)
    }
    fn render(&self, _: &mut ratatui::Frame, _: Rect, _: &mut crate::RenderCtx<'_>) {}
    fn take_pending_clipboard_request(&mut self) -> Option<String> {
        self.0.take()
    }
}

#[test]
fn background_clipboard_notifies_only_after_write_and_reports_write_failure() {
    crate::init();
    for fail in [false, true] {
        let notifications = Rc::new(RefCell::new(Vec::new()));
        let observed = notifications.clone();
        let mut app = TreeApp::new(Box::new(CopyRoot(Some("Sprint report".into()))))
            .on_notification(move |_, notification, _| observed.borrow_mut().push(notification));
        let mut flags = RuntimeFlags::default();
        app.flush_pending_clipboard(&mut flags, |text| {
            assert_eq!(text, "Sprint report");
            assert!(notifications.borrow().is_empty());
            if fail {
                Err(std::io::Error::other("terminal write failed"))
            } else {
                Ok(())
            }
        });
        assert_eq!(notifications.borrow().len(), 1);
        assert_eq!(
            notifications.borrow()[0].title(),
            if fail {
                "Copy failed"
            } else {
                "Copied to clipboard"
            }
        );
        assert!(flags.redraw);
        app.flush_pending_clipboard(&mut flags, |_| {
            panic!("Clipboard request must be consumed once")
        });
        assert_eq!(notifications.borrow().len(), 1);
    }
}
