use gpui::{div, prelude::*, Context, Entity};
use gpui_component::{
    button::Button,
    input::{Input, InputState},
    switch::Switch,
};

use super::root::RootView;
use super::state::{AppState, View};

fn root_chip(idx: usize, root: &std::path::Path, cx: &mut Context<RootView>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .child(div().child(root.display().to_string()))
        .child(Button::new(("remove-root", idx)).label("x").on_click(cx.listener(
            move |view, _event, _window, cx| {
                if idx < view.state.draft_roots.len() {
                    view.state.draft_roots.remove(idx);
                }
                cx.notify();
            },
        )))
}

fn rule_toggle(id: &'static str, label: &str, enabled: bool, cx: &mut Context<RootView>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .py_2()
        .child(div().child(label.to_string()))
        .child(
            Switch::new(id)
                .checked(enabled)
                .on_click(cx.listener(move |view, checked: &bool, _window, cx| {
                    view.state.draft_rule_enabled.insert(id, *checked);
                    cx.notify();
                })),
        )
}

pub fn render_settings(
    state: &AppState,
    new_root_input: &Entity<InputState>,
    cx: &mut Context<RootView>,
) -> impl IntoElement {
    let home = dirs::home_dir().expect("no home directory");
    let rules = crate::config::rules(&home, &[]);

    div()
        .flex()
        .flex_col()
        .size_full()
        .child(div().text_size(gpui::px(15.)).px_5().py_3().child("Search roots"))
        .child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .gap_2()
                .px_5()
                .children(state.draft_roots.iter().enumerate().map(|(idx, r)| root_chip(idx, r, cx))),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_2()
                .px_5()
                .py_2()
                .child(Input::new(new_root_input).id("new-root"))
                .child(Button::new("add-root").label("Add").on_click(cx.listener(
                    |view, _event, window, cx| {
                        let input = view.state.new_root_input.trim().to_string();
                        if !input.is_empty() {
                            let expanded = if let Some(rest) = input.strip_prefix("~/") {
                                dirs::home_dir().expect("no home directory").join(rest)
                            } else {
                                std::path::PathBuf::from(&input)
                            };
                            view.state.draft_roots.push(expanded);
                            view.state.new_root_input.clear();
                            let entity = view.new_root_input.clone();
                            entity.update(cx, |input_state, cx| {
                                input_state.set_value("", window, cx);
                            });
                        }
                        cx.notify();
                    },
                ))),
        )
        .child(div().text_size(gpui::px(15.)).px_5().py_3().child("Rules"))
        .child(
            div()
                .id("rules-scroll")
                .flex()
                .flex_col()
                .px_5()
                .flex_1()
                .min_h(gpui::px(0.))
                .overflow_y_scroll()
                .children(rules.iter().map(|r| {
                    let enabled = *state.draft_rule_enabled.get(r.id).unwrap_or(&true);
                    rule_toggle(r.id, r.description, enabled, cx)
                })),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .justify_end()
                .gap_2()
                .px_5()
                .py_3()
                .child(Button::new("settings-cancel").label("Cancel").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.draft_roots = view.state.roots.clone();
                        view.state.draft_rule_enabled = view.state.rule_enabled.clone();
                        view.state.view = View::Main;
                        cx.notify();
                    },
                )))
                .child(Button::new("settings-save").label("Save").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.roots = view.state.draft_roots.clone();
                        view.state.rule_enabled = view.state.draft_rule_enabled.clone();
                        view.state.view = View::Main;
                        view.state.scanning = true;
                        cx.notify();

                        let snapshot = view.state.rescan_inputs();
                        cx.spawn(async move |view, cx| {
                            let groups = cx.background_spawn(async move { snapshot.run() }).await;
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
}
