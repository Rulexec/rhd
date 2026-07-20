use crate::templates::TemplateRegistry;

pub struct TemplateLoader;

impl TemplateLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn get_template(&self, name: &str) -> Option<&'static str> {
        TemplateRegistry::get(name)
    }
}

#[cfg(test)]
mod tests;
