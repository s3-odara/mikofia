use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ConfigFormat {
    #[default]
    Ts,
    Js,
    Json,
}

impl ConfigFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ConfigFormat::Ts => "ts",
            ConfigFormat::Js => "js",
            ConfigFormat::Json => "json",
        }
    }

    pub fn default_filename(&self) -> String {
        format!("mikofia.config.{}", self.extension())
    }
}

/// テンプレート文字列を生成する純関数
pub fn get_template(format: ConfigFormat) -> String {
    match format {
        ConfigFormat::Ts => typescript_template(),
        ConfigFormat::Js => javascript_template(),
        ConfigFormat::Json => json_template(),
    }
}

fn typescript_template() -> String {
    r#"/** @type {import('mikofia').Config} */
export default {
  ignore: ["node_modules", ".git"],
  nodes: [
    { path: "src", existence: "required" },
    { path: "README.md", existence: "optional" }
  ]
};
"#
    .to_string()
}

fn javascript_template() -> String {
    r#"export default {
  ignore: ["node_modules", ".git"],
  nodes: [
    { path: "src", existence: "required" },
    { path: "README.md", existence: "optional" }
  ]
};
"#
    .to_string()
}

fn json_template() -> String {
    r#"{
  "ignore": ["node_modules", ".git"],
  "nodes": [
    {"path": "src", "existence": "required"},
    {"path": "README.md", "existence": "optional"}
  ]
}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_format_extension() {
        assert_eq!(ConfigFormat::Ts.extension(), "ts");
        assert_eq!(ConfigFormat::Js.extension(), "js");
        assert_eq!(ConfigFormat::Json.extension(), "json");
    }

    #[test]
    fn test_config_format_default_filename() {
        assert_eq!(ConfigFormat::Ts.default_filename(), "mikofia.config.ts");
        assert_eq!(ConfigFormat::Js.default_filename(), "mikofia.config.js");
        assert_eq!(ConfigFormat::Json.default_filename(), "mikofia.config.json");
    }

    #[test]
    fn test_typescript_template() {
        let template = get_template(ConfigFormat::Ts);
        assert!(template.contains("@type {import('mikofia').Config}"));
        assert!(template.contains("export default"));
        assert!(template.contains(r#""node_modules""#));
        assert!(template.contains(r#""src""#));
        assert!(template.contains(r#""required""#));
    }

    #[test]
    fn test_javascript_template() {
        let template = get_template(ConfigFormat::Js);
        assert!(!template.contains("@type"));
        assert!(template.contains("export default"));
        assert!(template.contains(r#""node_modules""#));
        assert!(template.contains(r#""src""#));
    }

    #[test]
    fn test_json_template() {
        let template = get_template(ConfigFormat::Json);
        assert!(template.contains(r#""ignore""#));
        assert!(template.contains(r#""nodes""#));
        assert!(template.contains(r#""node_modules""#));
        // JSONとして有効かを確認
        let parsed: serde_json::Value = serde_json::from_str(&template).unwrap();
        assert!(parsed.get("ignore").is_some());
        assert!(parsed.get("nodes").is_some());
    }
}
