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

            cx.text_system()
                .add_fonts(vec![
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice(),
                    ),
                    std::borrow::Cow::Borrowed(
                        include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf").as_slice(),
                    ),
                ])
                .expect("failed to load embedded fonts");

            if let Err(err) = gpui_component::ThemeRegistry::global_mut(cx)
                .load_themes_from_str(include_str!("../theme.json"))
            {
                eprintln!("failed to load theme.json: {err}");
            }
            if let Some(dark_theme) = gpui_component::ThemeRegistry::global(cx)
                .themes()
                .get("Cleaner Dark")
                .cloned()
            {
                gpui_component::Theme::global_mut(cx).apply_config(&dark_theme);
                gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);
            }

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
