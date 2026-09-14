use gpui::{div, prelude::*, px, App};
use gpui_component::ActiveTheme;

/// A toolbar strip below the native title bar: the OS already draws the
/// real traffic-light buttons, so this only carries the per-screen title
/// and a trailing nav action.
pub fn title_bar(title: impl Into<gpui::SharedString>, cx: &App, trailing: impl IntoElement) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_3()
        .py_2()
        .bg(theme.title_bar)
        .border_b_1()
        .border_color(theme.title_bar_border)
        .child(div().w(px(40.)))
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
        .child(trailing)
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
