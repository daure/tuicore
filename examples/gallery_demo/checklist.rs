use tuicore::{Checklist, TreeAdapter};

use crate::Msg;

#[derive(Clone)]
pub(crate) struct ChecklistItem {
    id: usize,
    parent: Option<usize>,
    label: String,
}

pub(crate) type ChecklistShowcase = Checklist<ChecklistItem, usize, Msg>;

pub(crate) fn release_checklist() -> ChecklistShowcase {
    let items = [
        (1, None, "Prepare release"),
        (2, Some(1), "Run full test suite"),
        (3, Some(1), "Update changelog"),
        (4, Some(1), "Package artifacts"),
        (5, None, "Publish release"),
        (6, Some(5), "Publish to crates.io"),
        (7, Some(5), "Create GitHub release"),
        (8, None, "Announce release"),
    ]
    .into_iter()
    .map(|(id, parent, label)| ChecklistItem {
        id,
        parent,
        label: label.to_string(),
    })
    .collect::<Vec<_>>();
    let mut next_id = items.len() + 1;

    Checklist::new(
        items,
        |item| item.id,
        |item| item.label.clone(),
        move |label, _| {
            let item = ChecklistItem {
                id: next_id,
                parent: None,
                label,
            };
            next_id += 1;
            item
        },
    )
    .title("Enter check · + sibling · \\ child · Ctrl+X remove · <> depth · Ctrl+M move")
    .hotkey("ck")
    .tree(TreeAdapter::mutable_parent_id(
        |item: &ChecklistItem| item.parent,
        |item, parent| item.parent = parent,
    ))
    .expanded([1, 5])
    .checked([2, 3, 6])
    .cascade_descendants(true)
    .max_rows(8)
}
