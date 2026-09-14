use gpui::{div, AppContext as _, Entity, IntoElement, ParentElement, Render, Styled as _, Subscription};
use gpui_component::input::{InputEvent, InputState};

use super::state::AppState;

pub struct RootView {
    pub state: AppState,
    pub new_root_input: Entity<InputState>,
    _new_root_input_sub: Subscription,
}

impl RootView {
    pub fn new(window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> Self {
        let new_root_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Add a search root, e.g. ~/Downloads")
        });
        let _new_root_input_sub = cx.subscribe(&new_root_input, |view, input, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                view.state.new_root_input = input.read(cx).value().to_string();
            }
        });

        Self {
            state: AppState::new(),
            new_root_input,
            _new_root_input_sub,
        }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        match self.state.view {
            super::state::View::Main => super::main_view::render_main(&self.state, cx).into_any_element(),
            super::state::View::Settings => {
                super::settings_view::render_settings(&self.state, &self.new_root_input, cx).into_any_element()
            }
            super::state::View::ConfirmDelete => {
                let main = super::main_view::render_main(&self.state, cx).into_any_element();
                div()
                    .relative()
                    .size_full()
                    .child(main)
                    .child(super::confirm_dialog::render_confirm(&self.state, cx).into_any_element())
                    .into_any_element()
            }
        }
    }
}
