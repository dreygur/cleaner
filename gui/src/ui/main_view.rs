use gpui::{div, prelude::*, Context};
use gpui_component::{button::Button, checkbox::Checkbox};

use super::root::RootView;
use super::state::AppState;
use crate::format::human_bytes;
use crate::types::Group;

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
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_5()
        .py_2()
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
                .child(div().font_weight(gpui::FontWeight::SEMIBOLD).child(group.rule.description.to_string()))
                .child(div().text_size(gpui::px(11.)).child(format!("{} / {}", group.rule.id, group.rule.regen))),
        )
        .child(div().child(human_bytes(group.size)))
}

pub fn render_main(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let (count, selected_bytes) = selected_total(state);

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_end()
                .px_5()
                .py_4()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_size(gpui::px(27.)).child(human_bytes(total_size(state))))
                        .child(div().text_size(gpui::px(11.)).child("reclaimable")),
                )
                .child(Button::new("rescan").label("Rescan").on_click(cx.listener(
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
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .children(state.groups.iter().map(|g| row(g, state.selected.contains(g.rule.id), cx))),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_5()
                .py_3()
                .child(div().child(format!("{count} of {} groups selected, {}", state.groups.len(), human_bytes(selected_bytes))))
                .child(Button::new("clean-selected").label("Clean Selected...").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.view = super::state::View::ConfirmDelete;
                        cx.notify();
                    },
                ))),
        )
}
