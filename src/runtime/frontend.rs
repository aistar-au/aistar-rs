use super::mode::RuntimeMode;

pub trait FrontendAdapter<M: RuntimeMode> {
    fn poll_user_input(&mut self) -> Option<String>;
    fn render(&mut self, mode: &M);
    fn should_quit(&self) -> bool;
}
