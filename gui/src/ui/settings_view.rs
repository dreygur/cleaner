use gpui::{div, prelude::*, px, Context, Entity};
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    switch::Switch,
    ActiveTheme,
};

use super::chrome::title_bar;
use super::root::RootView;
use super::state::{AppState, View};
use crate::types::Kind;

fn group_label(label: impl Into<gpui::SharedString>, cx: &mut Context<RootView>) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .px_5()
        .pt_4()
        .pb_1()
        .text_size(px(11.))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(theme.muted_foreground)
        .child(label.into())
}

fn root_chip(idx: usize, root: &std::path::Path, cx: &mut Context<RootView>) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .pl_3()
        .pr_1()
        .py_1()
        .bg(theme.muted)
        .border_1()
        .border_color(theme.border)
        .rounded_md()
        .child(
            div()
                .font_family(theme.mono_font_family.clone())
                .text_size(px(12.))
                .text_color(theme.foreground)
                .child(root.display().to_string()),
        )
        .child(
            Button::new(("remove-root", idx))
                .ghost()
                .label("×")
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    if idx < view.state.draft_roots.len() {
                        view.state.draft_roots.remove(idx);
                    }
                    cx.notify();
                })),
        )
}

fn rule_toggle(id: &'static str, label: &str, enabled: bool, cx: &mut Context<RootView>) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .flex()
        .flex_row()
        .justify_between()
        .items_center()
        .px_5()
        .py_2()
        .border_b_1()
        .border_color(theme.border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(div().text_size(px(12.5)).font_weight(gpui::FontWeight::SEMIBOLD).text_color(theme.foreground).child(label.to_string()))
                .child(
                    div()
                        .font_family(theme.mono_font_family.clone())
                        .text_size(px(11.))
                        .text_color(theme.muted_foreground)
                        .child(id),
                ),
        )
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
    let artifacts: Vec<_> = rules.iter().filter(|r| r.kind == Kind::Artifact).collect();
    let caches: Vec<_> = rules.iter().filter(|r| r.kind == Kind::Fixed && r.id != "trash").collect();
    let trash: Vec<_> = rules.iter().filter(|r| r.id == "trash").collect();

    let theme = cx.theme();
    let background = theme.background;
    let border_color = theme.border;
    let muted = theme.muted;

    div()
        .flex()
        .flex_col()
        .size_full()
        .bg(background)
        .child(title_bar(
            "Settings",
            cx,
            Button::new("settings-back").ghost().label("←").on_click(cx.listener(
                |view, _event, _window, cx| {
                    view.state.draft_roots = view.state.roots.clone();
                    view.state.draft_rule_enabled = view.state.rule_enabled.clone();
                    view.state.view = View::Main;
                    cx.notify();
                },
            )),
        ))
        .child(
            div()
                .id("settings-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .child(group_label("Search roots", cx))
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
                        .child(Button::new("add-root").outline().label("Add").on_click(cx.listener(
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
                .child(group_label("Build artifacts", cx))
                .children(artifacts.iter().map(|r| {
                    let enabled = *state.draft_rule_enabled.get(r.id).unwrap_or(&true);
                    rule_toggle(r.id, r.description, enabled, cx)
                }))
                .child(group_label("Package & tool caches", cx))
                .children(caches.iter().map(|r| {
                    let enabled = *state.draft_rule_enabled.get(r.id).unwrap_or(&true);
                    rule_toggle(r.id, r.description, enabled, cx)
                }))
                .child(group_label("Trash", cx))
                .children(trash.iter().map(|r| {
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
                .bg(muted)
                .border_t_1()
                .border_color(border_color)
                .child(Button::new("settings-cancel").outline().label("Cancel").on_click(cx.listener(
                    |view, _event, _window, cx| {
                        view.state.draft_roots = view.state.roots.clone();
                        view.state.draft_rule_enabled = view.state.rule_enabled.clone();
                        view.state.view = View::Main;
                        cx.notify();
                    },
                )))
                .child(Button::new("settings-save").primary().label("Save").on_click(cx.listener(
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
