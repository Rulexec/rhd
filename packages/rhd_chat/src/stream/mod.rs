mod contract;
mod send;

#[cfg(test)]
mod tests;

pub use contract::inject_todo_tool_contract;
pub use send::{edit_and_resend, send_message};

use std::sync::Arc;

pub struct TemplateLoaderRef {
    get_template: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
}

impl TemplateLoaderRef {
    pub fn new<F>(get_template: F) -> Self
    where
        F: Fn(&str) -> Option<String> + Send + Sync + 'static,
    {
        Self {
            get_template: Arc::new(get_template),
        }
    }

    pub fn get_template(&self, name: &str) -> Option<String> {
        (self.get_template)(name)
    }
}
