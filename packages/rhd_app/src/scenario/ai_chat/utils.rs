pub fn apply_model_aliases(model_name: &str, model_aliases: &[(String, String)]) -> String {
    model_aliases
        .iter()
        .find(|(alias, _)| alias == model_name)
        .map(|(_, target)| target.clone())
        .unwrap_or_else(|| model_name.to_string())
}
