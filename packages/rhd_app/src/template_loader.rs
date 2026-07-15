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
mod tests {
    use super::*;

    #[test]
    fn test_get_existing_template() {
        let loader = TemplateLoader::new();
        assert!(loader.get_template("todo_list_empty").is_some());
        assert!(loader.get_template("todo_list_with_items").is_some());
        assert!(loader.get_template("environment_details_no_role").is_some());
        assert!(loader.get_template("environment_details_with_role").is_some());
        assert!(loader.get_template("rhd_set_todo_list_contract").is_some());
    }

    #[test]
    fn test_get_nonexistent_template() {
        let loader = TemplateLoader::new();
        assert!(loader.get_template("nonexistent_template").is_none());
    }
}
