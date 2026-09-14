use gpui::{div, prelude::*, px, Context};
use gpui_component::{button::Button, button::ButtonVariants, checkbox::Checkbox, ActiveTheme};

use super::chrome::{section_head, title_bar};
use super::root::RootView;
use super::state::AppState;
use crate::format::human_bytes;
use crate::types::{Group, Kind};

fn total_size(state: &AppState) -> u64 {
    state.groups.iter().map(|g| g.size).sum()
}

fn selected_total(state: &AppState) -> (usize, u64) {
    let selected: Vec<&Group> = state
        .groups
        .iter()
        .filter(|g| state.selected.contains(g.rule.id))
        .collect();
    let size = selected.iter().map(|g| g.size).sum();
    (selected.len(), size)
}

fn row(group: &Group, checked: bool, cx: &mut Context<RootView>) -> impl IntoElement {
    let id = group.rule.id;
    let is_trash = group.rule.id == "trash";
    let theme = cx.theme();
    let row_bg = if is_trash { theme.danger.alpha(0.08) } else { theme.background };
    let hover_bg = if is_trash { theme.danger.alpha(0.14) } else { theme.muted };
    let border_color = theme.border;
    let foreground = theme.foreground;
    let muted_foreground = theme.muted_foreground;
    let danger = theme.danger;
    let danger_bg = theme.danger.alpha(0.14);
    let mono_font = theme.mono_font_family.clone();

    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_5()
        .py_2()
        .bg(row_bg)
        .border_b_1()
        .border_color(border_color)
        .hover(move |el| el.bg(hover_bg))
        .child(
            Checkbox::new(id)
                .checked(checked)
                .on_click(cx.listener(move |view, checked: &bool, _window, cx| {
                    if *checked {
                        view.state.selected.insert(id);
                    } else {
                        view.state.selected.remove(id);
                    }
                    cx.notify();
                })),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap_1()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_baseline()
                        .gap_2()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(foreground)
                                .child(group.rule.description.to_string()),
                        )
                        .child(
                            div()
                                .font_family(mono_font.clone())
                                .text_size(px(11.))
                                .text_color(muted_foreground)
                                .child(group.rule.id),
                        )
                        .when(is_trash, |el| {
                            el.child(
                                div()
                                    .text_size(px(9.5))
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(danger)
                                    .bg(danger_bg)
                                    .rounded_sm()
                                    .px_1()
                                    .child("PERMANENT"),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(muted_foreground)
                        .child(format!("regenerates: {}", group.rule.regen)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap_1()
                .child(
                    div()
                        .font_family(mono_font.clone())
                        .text_size(px(13.5))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(foreground)
                        .child(human_bytes(group.size)),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(muted_foreground)
                        .child(format!("{} path{}", group.targets.len(), if group.targets.len() == 1 { "" } else { "s" })),
                ),
        )
}

fn grouped_rows(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let artifacts: Vec<&Group> = state.groups.iter().filter(|g| g.rule.kind == Kind::Artifact).collect();
    let caches: Vec<&Group> = state
        .groups
        .iter()
        .filter(|g| g.rule.kind == Kind::Fixed && g.rule.id != "trash")
        .collect();
    let trash: Vec<&Group> = state.groups.iter().filter(|g| g.rule.id == "trash").collect();

    div()
        .id("groups-scroll")
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .overflow_y_scroll()
        .when(!artifacts.is_empty(), |el| {
            el.child(section_head("Build artifacts · rebuilds automatically", cx))
                .children(artifacts.into_iter().map(|g| row(g, state.selected.contains(g.rule.id), cx)))
        })
        .when(!caches.is_empty(), |el| {
            el.child(section_head("Package & tool caches", cx))
                .children(caches.into_iter().map(|g| row(g, state.selected.contains(g.rule.id), cx)))
        })
        .when(!trash.is_empty(), |el| {
            el.child(section_head("Trash", cx))
                .children(trash.into_iter().map(|g| row(g, state.selected.contains(g.rule.id), cx)))
        })
}

pub fn render_main(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let (count, selected_bytes) = selected_total(state);
    let roots_line = state
        .roots
        .iter()
        .map(|r| r.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let theme = cx.theme();
    let background = theme.background;
    let border_color = theme.border;
    let foreground = theme.foreground;
    let muted_foreground = theme.muted_foreground;
    let muted = theme.muted;
    let warning = theme.warning;
    let warning_bg = theme.warning.alpha(0.12);
    let mono_font = theme.mono_font_family.clone();

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(background)
        .child(title_bar(
            "Cleaner",
            cx,
            Button::new("open-settings").ghost().label("⚙︎").on_click(cx.listener(
                |view, _event, _window, cx| {
                    view.state.view = super::state::View::Settings;
                    cx.notify();
                },
            )),
        ))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_end()
                .px_5()
                .py_4()
                .border_b_1()
                .border_color(border_color)
                .gap_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w(px(0.))
                        .gap_1()
                        .child(
                            div()
                                .text_size(px(11.))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(muted_foreground)
                                .child("Reclaimable"),
                        )
                        .child(
                            div()
                                .font_family(mono_font.clone())
                                .text_size(px(27.))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(foreground)
                                .child(human_bytes(total_size(state))),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.5))
                                .text_color(muted_foreground)
                                .child(format!("across {} rule groups · scanned {}", state.groups.len(), roots_line)),
                        ),
                )
                .child(Button::new("rescan").outline().label("Rescan").flex_shrink_0().on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.scanning = true;
                        cx.notify();

                        let snapshot = view.state.rescan_inputs();
                        cx.spawn(async move |view, cx| {
                            let groups = cx
                                .background_spawn(async move { snapshot.run() })
                                .await;
                            view.update(cx, |view, cx| {
                                view.state.groups = groups;
                                view.state.scanning = false;
                                cx.notify();
                            })
                            .ok();
                        })
                        .detach();
                    },
                ))),
        )
        .when_some(state.last_clean_error.as_ref(), |el, msg| {
            el.child(
                div()
                    .px_5()
                    .py_2()
                    .text_size(px(12.))
                    .text_color(warning)
                    .bg(warning_bg)
                    .child(msg.clone()),
            )
        })
        .child(grouped_rows(state, cx))
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_5()
                .py_3()
                .bg(muted)
                .border_t_1()
                .border_color(border_color)
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(muted_foreground)
                        .child(format!(
                            "{count} of {} groups selected · {}",
                            state.groups.len(),
                            human_bytes(selected_bytes)
                        )),
                )
                .child(Button::new("clean-selected").primary().label("Clean Selected...").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.view = super::state::View::ConfirmDelete;
                        cx.notify();
                    },
                ))),
        )
}
