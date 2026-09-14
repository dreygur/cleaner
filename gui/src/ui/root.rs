use gpui::{div, IntoElement, ParentElement, Render, Styled};

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
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full().child("Cleaner")
    }
}
