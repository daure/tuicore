use tuicore::{
    Flex, FlexItem, Grid, GridItem, GridTrack, Overlay, OverlayAnchor, OverlaySize, Padding, Panel,
    Paragraph, Separator, SeparatorColorRole, Split, Stack, StackAlign, StackItem, TreeApp,
};

fn main() -> tuicore::Result<()> {
    tuicore::init();

    let navigation = Flex::<()>::column()
        .gap(1)
        .padding(Padding::all(1))
        .child(
            "overview",
            Paragraph::new("Overview"),
            FlexItem::fit_content(),
        )
        .child(
            "services",
            Paragraph::new("Services"),
            FlexItem::fit_content(),
        )
        .child("spacer", Paragraph::new(""), FlexItem::fill(1))
        .child(
            "hint",
            Paragraph::new("Ctrl+Q exits"),
            FlexItem::fit_content(),
        );

    let dashboard = Grid::<()>::new()
        .columns([GridTrack::fit_content(), GridTrack::fill(1)])
        .rows([GridTrack::fit_content(), GridTrack::fill(1)])
        .gap(1, 1)
        .padding(Padding::all(1))
        .child(
            "health",
            Panel::new()
                .top_left("Health")
                .host(Paragraph::new("● All systems operational")),
            GridItem::new(0, 0),
        )
        .child(
            "summary",
            Panel::new()
                .top_left("Summary")
                .host(Paragraph::new("12 services  ·  3 regions  ·  99.99% uptime")),
            GridItem::new(0, 1),
        )
        .child(
            "activity",
            Panel::new().top_left("Activity").host(Paragraph::new(
                "09:42  API deployment completed\n09:36  Worker pool scaled to 8\n09:20  Backups verified",
            )),
            GridItem::new(1, 0).span(1, 2),
        );

    let dashboard_with_popover = Overlay::new(
        Panel::new().top_left("Dashboard").host(dashboard),
        Panel::new()
            .top_left("Overlay")
            .host(Paragraph::new("Anchored popover\nFit to content")),
    )
    .anchor(OverlayAnchor::BottomRight)
    .layer_size(OverlaySize::FitContent);

    let workspace = Stack::<()>::new()
        .child("content", dashboard_with_popover, StackItem::new())
        .child(
            "live",
            Paragraph::new("● LIVE"),
            StackItem::new()
                .fit_content()
                .align(StackAlign::End, StackAlign::Start)
                .inset(Padding::all(2)),
        );

    let body = Split::horizontal(
        Panel::new().top_left("Navigation").host(navigation),
        workspace,
    )
    .ratio(1, 4)
    .gap(1)
    .separator(Separator::new().role(SeparatorColorRole::Subtle));

    let root = Flex::<()>::column()
        .padding(Padding::all(1))
        .gap(1)
        .child(
            "header",
            Paragraph::new("TUICORE OPERATIONS"),
            FlexItem::fit_content(),
        )
        .child("body", body, FlexItem::fill(1))
        .child(
            "footer",
            Paragraph::new("Flex flow · Grid tracks · Split panes · Stack and Overlay layers"),
            FlexItem::fit_content(),
        );

    TreeApp::new(root).run()
}
