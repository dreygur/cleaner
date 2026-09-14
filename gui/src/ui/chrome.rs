use gpui::{div, prelude::*, px, rgb, App};
use gpui_component::ActiveTheme;

const TRAFFIC_RED: u32 = 0xff5f57;
const TRAFFIC_YELLOW: u32 = 0xfebc2e;
const TRAFFIC_GREEN: u32 = 0x28c840;

fn traffic_dot(color: u32) -> impl IntoElement {
    div().w(px(11.)).h(px(11.)).rounded_full().bg(rgb(color))
}

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
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .child(traffic_dot(TRAFFIC_RED))
                .child(traffic_dot(TRAFFIC_YELLOW))
                .child(traffic_dot(TRAFFIC_GREEN)),
        )
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
