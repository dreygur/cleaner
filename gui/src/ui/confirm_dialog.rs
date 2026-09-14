use gpui::{div, prelude::*, Context};
use gpui_component::button::Button;

use super::root::RootView;
use super::state::{AppState, View};
use crate::format::human_bytes;
use crate::types::Group;

fn selected_groups(state: &AppState) -> Vec<&Group> {
    state.groups.iter().filter(|g| state.selected.contains(g.rule.id)).collect()
}

pub fn render_confirm(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let groups = selected_groups(state);
    let total: u64 = groups.iter().map(|g| g.size).sum();
    let has_trash = groups.iter().any(|g| g.rule.id == "trash");

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt_16()
        .child(
            div()
                .w(gpui::px(400.))
                .flex()
                .flex_col()
                .child(div().text_size(gpui::px(15.)).px_4().pt_4().child(format!("Delete {} groups?", groups.len())))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .px_4()
                        .py_2()
                        .children(groups.iter().map(|g| {
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .child(div().child(g.rule.description.to_string()))
                                .child(div().child(human_bytes(g.size)))
                        })),
                )
                .when(has_trash, |el| {
                    el.child(
                        div()
                            .px_4()
                            .py_2()
                            .child("Trash is a permanent delete, those items will not be recoverable."),
                    )
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .px_4()
                        .py_2()
                        .child(div().child("Total"))
                        .child(div().child(human_bytes(total))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .child(Button::new("confirm-cancel").label("Cancel").on_click(cx.listener(
                            |view, _event, _window, cx| {
                                view.state.view = View::Main;
                                cx.notify();
                            },
                        )))
                        .child(Button::new("confirm-delete").label("Delete Selected").on_click(cx.listener(
                            |view, _event, _window, cx| {
                                let home = dirs::home_dir().expect("no home directory");
                                let to_delete: Vec<Group> = view
                                    .state
                                    .groups
                                    .iter()
                                    .filter(|g| view.state.selected.contains(g.rule.id))
                                    .cloned()
                                    .collect();
                                let snapshot = view.state.rescan_inputs();

                                cx.spawn(async move |view, cx| {
                                    let home = home;
                                    let groups = cx
                                        .background_spawn(async move {
                                            for group in &to_delete {
                                                crate::clean::clean_group(&home, group);
                                            }
                                            snapshot.run()
                                        })
                                        .await;
                                    view.update(cx, |view, cx| {
                                        view.state.groups = groups;
                                        view.state.view = View::Main;
                                        cx.notify();
                                    })
                                    .ok();
                                })
                                .detach();
                            },
                        ))),
                ),
        )
}
