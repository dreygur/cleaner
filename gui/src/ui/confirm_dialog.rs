use gpui::{div, prelude::*, px, Context};
use gpui_component::{button::Button, button::ButtonVariants, ActiveTheme, Disableable};

use super::root::RootView;
use super::state::{AppState, View};
use crate::config::RULE_TRASH;
use crate::format::human_bytes;
use crate::types::Group;

fn selected_groups(state: &AppState) -> Vec<&Group> {
    state.groups.iter().filter(|g| state.selected.contains(g.rule.id)).collect()
}

pub fn render_confirm(state: &AppState, cx: &mut Context<RootView>) -> impl IntoElement {
    let theme = cx.theme();
    let groups = selected_groups(state);
    let total: u64 = groups.iter().map(|g| g.size).sum();
    let has_trash = groups.iter().any(|g| g.rule.id == RULE_TRASH);
    let deleting = state.deleting;

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_start()
        .justify_center()
        .pt_16()
        .bg(theme.overlay)
        .child(
            div()
                .w(px(400.))
                .flex()
                .flex_col()
                .bg(theme.popover)
                .border_1()
                .border_color(theme.border)
                .rounded_lg()
                .shadow_lg()
                .child(
                    div()
                        .px_4()
                        .pt_4()
                        .pb_1()
                        .child(
                            div()
                                .text_size(px(15.))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.foreground)
                                .child(format!("Delete {} groups?", groups.len())),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("This frees space immediately. Build artifacts and caches listed below regenerate on demand."),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .px_4()
                        .py_2()
                        .children(groups.iter().map(|g| {
                            div()
                                .flex()
                                .flex_row()
                                .justify_between()
                                .items_baseline()
                                .text_size(px(12.5))
                                .child(div().text_color(theme.foreground).child(g.rule.description.to_string()))
                                .child(
                                    div()
                                        .font_family(theme.mono_font_family.clone())
                                        .text_color(theme.muted_foreground)
                                        .child(human_bytes(g.size)),
                                )
                        })),
                )
                .when(has_trash, |el| {
                    el.child(
                        div()
                            .mx_4()
                            .mb_2()
                            .px_2()
                            .py_2()
                            .rounded_md()
                            .bg(theme.warning.alpha(0.14))
                            .text_size(px(11.5))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.warning)
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
                        .border_t_1()
                        .border_color(theme.border)
                        .text_size(px(13.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child(div().text_color(theme.foreground).child("Total"))
                        .child(
                            div()
                                .font_family(theme.mono_font_family.clone())
                                .text_color(theme.primary)
                                .child(human_bytes(total)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_end()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .bg(theme.muted)
                        .child(
                            Button::new("confirm-cancel")
                                .outline()
                                .label("Cancel")
                                .disabled(deleting)
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    if view.state.deleting {
                                        return;
                                    }
                                    view.state.view = View::Main;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("confirm-delete")
                                .danger()
                                .label("Delete Selected")
                                .disabled(deleting)
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    if view.state.deleting {
                                        return;
                                    }
                                    view.state.deleting = true;
                                    cx.notify();

                                    let home = dirs::home_dir().expect("no home directory");
                                    let to_delete: Vec<Group> = view
                                        .state
                                        .groups
                                        .iter()
                                        .filter(|g| view.state.selected.contains(g.rule.id))
                                        .cloned()
                                        .collect();
                                    let snapshot = view.state.rescan_inputs();
                                    let total_groups = to_delete.len();

                                    cx.spawn(async move |view, cx| {
                                        let home = home;
                                        let (groups, failed_groups, failures) = cx
                                            .background_spawn(async move {
                                                let mut failures = Vec::new();
                                                let mut failed_groups = 0usize;
                                                for group in &to_delete {
                                                    let (_freed, errs) =
                                                        crate::clean::clean_group(&home, group);
                                                    if !errs.is_empty() {
                                                        failed_groups += 1;
                                                        failures.extend(errs);
                                                    }
                                                }
                                                (snapshot.run(), failed_groups, failures)
                                            })
                                            .await;
                                        view.update(cx, |view, cx| {
                                            view.state.groups = groups;
                                            view.state.view = View::Main;
                                            view.state.deleting = false;
                                            view.state.last_clean_error = if failures.is_empty() {
                                                None
                                            } else {
                                                Some(format!(
                                                    "{} of {} groups cleaned; {} failed: {}",
                                                    total_groups.saturating_sub(failed_groups),
                                                    total_groups,
                                                    failed_groups,
                                                    failures[0]
                                                ))
                                            };
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .detach();
                                })),
                        ),
                ),
        )
}
