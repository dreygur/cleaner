use gpui::{div, IntoElement, ParentElement, Render};

use super::state::AppState;

pub struct RootView {
    pub state: AppState,
}

impl RootView {
    pub fn new() -> Self {
        Self { state: AppState::new() }
    }
}

impl Render for RootView {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        match self.state.view {
            super::state::View::Main => super::main_view::render_main(&self.state, cx).into_any_element(),
            super::state::View::Settings => div().child("settings").into_any_element(),
            super::state::View::ConfirmDelete => div().child("confirm").into_any_element(),
        }
    }
}
