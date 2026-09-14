mod format;
mod error;
mod types;
mod config;
mod scan;
mod clean;
mod ui;

fn main() {
    use gpui::AppContext as _;

    gpui_platform::application()
        .with_assets(gpui_kit_assets::Assets)
        .run(|cx: &mut gpui::App| {
            gpui_component::init(cx);

            let bounds = gpui::Bounds::centered(
                None,
                gpui::size(gpui::px(760.), gpui::px(560.)),
                cx,
            );

            let window_options = gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                ..gpui_component::TitleBar::window_options()
            };

            cx.open_window(window_options, |window, cx| {
                window.set_window_title("Cleaner");
                let view = cx.new(|cx| ui::root::RootView::new(window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            })
            .unwrap();
        });
}
