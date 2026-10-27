//! One catalog shared by the UI, app, and isolated summary engine.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub prompt: String,
    pub markdown: bool,
    pub summary_tokens: usize,
    pub notes_tokens: usize,
    pub context_tokens: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    default_id: String,
    templates: Vec<Template>,
}

fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../src/summaryTemplates.json"))
            .expect("valid bundled summary templates")
    })
}

pub fn default_id() -> String {
    catalog().default_id.clone()
}

pub fn get(id: &str) -> Result<&'static Template, String> {
    catalog()
        .templates
        .iter()
        .find(|template| template.id == id)
        .ok_or_else(|| format!("unknown summary template: {id}"))
}

impl Template {
    // Built-in output layouts belong only in the final pass. Small local models
    // otherwise write miniature reports instead of retaining factual notes.
    pub fn note_instructions<'a>(&self, custom: Option<&'a str>) -> Option<&'a str> {
        if self.id == "custom" {
            custom.map(str::trim).filter(|value| !value.is_empty())
        } else {
            None
        }
    }

    pub fn instructions<'a>(&'a self, custom: Option<&'a str>) -> &'a str {
        if self.id == "custom" {
            custom
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&self.prompt)
        } else {
            &self.prompt
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_detailed_and_custom_prompts_do_not_override_presets() {
        let template = get(&default_id()).unwrap();
        assert_eq!(template.id, "discussion");
        assert!(template.markdown);
        assert!(template.summary_tokens >= 8000);
        assert!(template.context_tokens as usize > template.summary_tokens + template.notes_tokens);
        assert_eq!(
            template.instructions(Some("Ignore topics")),
            template.prompt
        );
        assert_eq!(
            get("custom").unwrap().instructions(Some("My instructions")),
            "My instructions"
        );
        assert_eq!(
            get("custom").unwrap().instructions(Some(" ")),
            get("custom").unwrap().prompt
        );
        assert!(get("missing").is_err());
        assert_eq!(
            template.note_instructions(Some("A saved custom prompt")),
            None
        );
        assert_eq!(
            get("custom")
                .unwrap()
                .note_instructions(Some("Focus on risks")),
            Some("Focus on risks")
        );
    }
}
