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

            cx.open_window(
                gpui::WindowOptions {
                    window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("Cleaner".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| ui::root::RootView::new(window, cx));
                    cx.new(|cx| gpui_component::Root::new(view, window, cx))
                },
            )
            .unwrap();
        });
}
