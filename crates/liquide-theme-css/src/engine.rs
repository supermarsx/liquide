//! Theme engine for applying CSS styles

use crate::cache::QueryCache;
use crate::error::Result;
use crate::parser::ThemeParser;
use crate::property::PropertySet;
use crate::stylesheet::{QueryEnvironment, StyleSheet};
use crate::value::PropertyValue;

/// Theme engine for querying and applying styles
pub struct ThemeEngine {
    stylesheet: StyleSheet,
    /// Query result cache
    cache: QueryCache,
}

impl ThemeEngine {
    /// Create a new theme engine with a stylesheet
    pub fn new(stylesheet: StyleSheet) -> Self {
        Self::with_cache_size(stylesheet, 1000)
    }

    /// Create a theme engine by parsing a CSS string
    pub fn from_css(css: &str) -> Result<Self> {
        let parser = ThemeParser::new();
        let stylesheet = parser.parse_str(css)?;
        Ok(Self::new(stylesheet))
    }

    /// Create a new theme engine with custom cache size
    ///
    /// # Arguments
    /// * `stylesheet` - The CSS stylesheet
    /// * `cache_size` - Maximum number of cached queries (0 = unlimited)
    pub fn with_cache_size(stylesheet: StyleSheet, cache_size: usize) -> Self {
        Self {
            stylesheet,
            cache: QueryCache::new(cache_size),
        }
    }

    /// Query styles for an element
    ///
    /// Returns the computed property set after applying CSS cascade rules.
    /// Results are cached for performance.
    pub fn query(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
    ) -> Result<PropertySet> {
        self.query_with_id(element, None, classes, pseudo_classes)
    }

    /// Query styles for an element in a specific query environment.
    ///
    /// This bypasses the default query cache because results depend on environment fields
    /// such as viewport and preferred color scheme.
    pub fn query_with_environment(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        env: &QueryEnvironment,
    ) -> Result<PropertySet> {
        self.query_with_id_and_environment(element, None, classes, pseudo_classes, env)
    }

    /// Query styles with ID
    ///
    /// Results are cached for performance.
    pub fn query_with_id(
        &self,
        element: &str,
        id: Option<&str>,
        classes: &[String],
        pseudo_classes: &[String],
    ) -> Result<PropertySet> {
        // Check cache first
        if let Some(properties) = self.cache.get(element, classes, id, pseudo_classes) {
            return Ok(properties);
        }

        // Compute styles
        let properties = self
            .stylesheet
            .compute_styles(element, classes, id, pseudo_classes);

        // Cache the result
        self.cache
            .insert(element, classes, id, pseudo_classes, properties.clone());

        Ok(properties)
    }

    /// Query styles with ID in a specific query environment.
    ///
    /// This bypasses cache because conditional rules can vary by environment.
    pub fn query_with_id_and_environment(
        &self,
        element: &str,
        id: Option<&str>,
        classes: &[String],
        pseudo_classes: &[String],
        env: &QueryEnvironment,
    ) -> Result<PropertySet> {
        Ok(self.stylesheet.compute_styles_with_environment(
            element,
            classes,
            id,
            pseudo_classes,
            env,
        ))
    }

    /// Get a specific property value
    pub fn get_property(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        property: &str,
    ) -> Result<Option<PropertyValue>> {
        let styles = self.query(element, classes, pseudo_classes)?;
        Ok(styles.get(property).cloned())
    }

    /// Check if element has a specific style
    pub fn has_property(
        &self,
        element: &str,
        classes: &[String],
        pseudo_classes: &[String],
        property: &str,
    ) -> bool {
        if let Ok(styles) = self.query(element, classes, pseudo_classes) {
            styles.has(property)
        } else {
            false
        }
    }

    /// Get a CSS variable
    pub fn get_variable(&self, name: &str) -> Option<&PropertyValue> {
        self.stylesheet.get_variable(name)
    }

    /// Get the underlying stylesheet
    pub fn stylesheet(&self) -> &StyleSheet {
        &self.stylesheet
    }

    /// Replace the stylesheet (for hot-reloading)
    ///
    /// Clears the query cache when stylesheet changes.
    pub fn set_stylesheet(&mut self, stylesheet: StyleSheet) {
        self.stylesheet = stylesheet;
        self.cache.clear();
    }

    /// Get cache statistics
    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats()
    }

    /// Clear the query cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Pre-warm cache with common queries
    ///
    /// # Example
    /// ```rust,ignore
    /// let common_queries = vec![
    ///     ("button".to_string(), vec![], None, vec![]),
    ///     ("button".to_string(), vec!["primary".to_string()], None, vec![]),
    ///     ("button".to_string(), vec!["primary".to_string()], None, vec!["hover".to_string()]),
    /// ];
    /// engine.prewarm_cache(&common_queries);
    /// ```
    pub fn prewarm_cache(&self, queries: &[(String, Vec<String>, Option<String>, Vec<String>)]) {
        self.cache
            .prewarm(queries, |element, classes, id, pseudo_classes| {
                self.stylesheet
                    .compute_styles(element, classes, id, pseudo_classes)
            });
    }
}

#[cfg(test)]
#[path = "tests/engine_tests.rs"]
mod tests;
