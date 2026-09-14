use gpui::{div, prelude::*, px, App};
use gpui_component::{ActiveTheme, TitleBar as GpuiTitleBar};

/// The app's custom title bar: real macOS traffic lights blended into our
/// own bar (via `GpuiTitleBar::window_options()` in main.rs), carrying the
/// per-screen title and a trailing nav action. Draggable and
/// double-click-to-zoom for free, courtesy of gpui-component's own widget.
pub fn title_bar(title: impl Into<gpui::SharedString>, cx: &App, trailing: impl IntoElement) -> impl IntoElement {
    let theme = cx.theme();
    GpuiTitleBar::new()
        .child(
            div()
                .flex_1()
                .flex()
                .justify_center()
                .text_size(px(12.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(title.into()),
        )
        .child(div().mr_2().child(trailing))
}

pub fn section_head(label: impl Into<gpui::SharedString>, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .px_5()
        .py_2()
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.muted_foreground)
        .bg(theme.muted)
        .child(label.into())
}
