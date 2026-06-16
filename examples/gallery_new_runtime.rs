#[derive(Debug, PartialEq)]
enum Msg {
    Quit,
}

fn main() -> tuicore::Result<()> {
    tuicore::init();
    let root = tuicore::Panel::new()
        .top_left("Tree runtime composition")
        .host(
            tuicore::Split::horizontal(
                tuicore::Flex::column()
                    .gap(1)
                    .padding(tuicore::Padding::all(1))
                    .child(
                        "tabs",
                        tuicore::Tabs::default().variant(tuicore::TabsVariant::Underline),
                        tuicore::FlexItem::fill(1),
                    )
                    .child(
                        "status",
                        tuicore::Panel::new().top_left("Status").content([
                            "Split is 30/70.",
                            "Tab/Shift-Tab moves focus.",
                            "Enter in search exits.",
                        ]),
                        tuicore::FlexItem::fixed(5),
                    ),
                tuicore::Panel::new().top_left("Search").host(
                    tuicore::TextInput::new()
                        .placeholder("Type text, Enter exits")
                        .on_submit(|_| Msg::Quit),
                ),
            )
            .ratio(30, 70),
        );

    tuicore::TreeApp::new(root)
        .on_message(|_, msg, ctx| match msg {
            Msg::Quit => ctx.request_quit(),
        })
        .run()
}
