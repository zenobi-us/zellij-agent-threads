//! Parses Zellij plugin configuration into typed runtime settings.
//!
//! Zellij gives plugins a flat `BTreeMap<String, String>` from KDL layout blocks
//! or `zellij plugin --configuration` flags. This module is the only place that
//! knows those string keys and fallback rules, so rendering and runtime code do
//! not grow ad-hoc parsing.

use std::collections::BTreeMap;

/// Complete typed configuration for the plugin.
///
/// Keep this struct boring: every field should be cheap to clone and safe to use
/// directly during render/update callbacks. Invalid user input is handled during
/// [`PluginConfig::parse`] by falling back to defaults instead of failing plugin
/// startup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PluginConfig {
    pub(crate) render: RenderConfig,
}

impl Default for PluginConfig {
    /// Provides safe defaults for layouts that launch the plugin with no config.
    fn default() -> Self {
        Self {
            render: RenderConfig::default(),
        }
    }
}

impl PluginConfig {
    /// Converts Zellij's flat string config into typed plugin settings.
    ///
    /// This function is intentionally forgiving. A bad optional value should not
    /// prevent Pi or Zellij from starting; it should degrade to the same behavior
    /// users get with no config.
    pub(crate) fn parse(configuration: &BTreeMap<String, String>) -> Self {
        Self {
            render: RenderConfig::parse(configuration),
        }
    }
}

/// Rendering-specific config parsed from the plugin config map.
///
/// This stays separate from [`PluginConfig`] because the render module should not
/// need to know about update-time settings like resize behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RenderConfig {
    pub(crate) title: String,
    pub(crate) empty_message: String,
    pub(crate) template: String,
    pub(crate) template_dir: Option<String>,
    pub(crate) template_name: String,
}

impl Default for RenderConfig {
    /// Provides labels used by the default compact status UI.
    fn default() -> Self {
        Self {
            title: "zellij-agent".into(),
            empty_message: "waiting for pi extension reports".into(),
            template: crate::render::DEFAULT_TEMPLATE.into(),
            template_dir: None,
            template_name: "main.j2".into(),
        }
    }
}

impl RenderConfig {
    /// Parses only the keys used by rendering.
    ///
    /// Keeping this constructor private prevents callers from depending on the
    /// raw KDL key names; callers should ask [`PluginConfig::parse`] for the
    /// full typed config instead.
    fn parse(configuration: &BTreeMap<String, String>) -> Self {
        let default = Self::default();
        Self {
            title: configuration.get("title").cloned().unwrap_or(default.title),
            empty_message: configuration
                .get("empty_message")
                .cloned()
                .unwrap_or(default.empty_message),
            template: configuration
                .get("template")
                .cloned()
                .unwrap_or(default.template),
            template_dir: configuration.get("template_dir").cloned(),
            template_name: configuration
                .get("template_name")
                .cloned()
                .unwrap_or(default.template_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_config_values() {
        let config = PluginConfig::parse(&BTreeMap::from([
            ("title".into(), "agents".into()),
            ("empty_message".into(), "none".into()),
            ("template".into(), "{{ status }}".into()),
        ]));

        assert_eq!(config.render.title, "agents");
        assert_eq!(config.render.empty_message, "none");
        assert_eq!(config.render.template, "{{ status }}");
        assert_eq!(config.render.template_dir, None);
        assert_eq!(config.render.template_name, "main.j2");
    }

    #[test]
    fn parses_template_loader_config() {
        let config = PluginConfig::parse(&BTreeMap::from([
            ("template_dir".into(), "/tmp/templates".into()),
            ("template_name".into(), "agent.j2".into()),
        ]));

        assert_eq!(
            config.render.template_dir.as_deref(),
            Some("/tmp/templates")
        );
        assert_eq!(config.render.template_name, "agent.j2");
    }
}
