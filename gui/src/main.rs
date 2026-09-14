mod format;
mod error;
mod types;
mod config;
mod scan;
mod clean;
mod ui;

fn main() {
    use gpui::AppContext as _;

    gpui_platform::application().run(|cx: &mut gpui::App| {
        gpui_component::init(cx);

        let bounds = gpui::Bounds::centered(
            None,
            gpui::size(gpui::px(760.), gpui::px(560.)),
            cx,
        );

        cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| ui::root::RootView::new());
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .unwrap();
    });
}
