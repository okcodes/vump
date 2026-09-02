//! Tag name templates.
//!
//! A pattern such as `api-v{new_version}` is used in both directions: to name
//! the tag a bump creates, and to recognize which project a pushed tag belongs
//! to and what version it claims. Reading tags is what lets CI verify a tag
//! without being told separately which project it refers to.
//!
//! Matching is deliberately literal rather than a regular expression. A pattern
//! is a fixed prefix, the version, and a fixed suffix, so there is nothing for a
//! user to get subtly wrong and no way for a pattern to match more than it
//! looks like it should.

use semver::Version;
use thiserror::Error;

/// The placeholder replaced by the version being released.
pub const VERSION_PLACEHOLDER: &str = "{new_version}";

/// The placeholder replaced by the project's name.
pub const PROJECT_PLACEHOLDER: &str = "{project}";

/// A tag name template, split around the version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagPattern {
    prefix: String,
    suffix: String,
}

/// Why a tag pattern is not usable.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TagPatternError {
    /// The pattern has no place to put the version.
    #[error("tag pattern {pattern:?} contains no {VERSION_PLACEHOLDER}")]
    NoVersion {
        /// The offending pattern.
        pattern: String,
    },

    /// The pattern would put the version in more than one place.
    #[error("tag pattern {pattern:?} contains {VERSION_PLACEHOLDER} more than once")]
    RepeatedVersion {
        /// The offending pattern.
        pattern: String,
    },
}

impl TagPattern {
    /// Interprets a template.
    ///
    /// # Errors
    ///
    /// Returns [`TagPatternError`] when the template does not contain exactly
    /// one version placeholder. A pattern without one names the same tag for
    /// every release; a pattern with several cannot be read back.
    pub fn parse(pattern: &str) -> Result<Self, TagPatternError> {
        let mut parts = pattern.split(VERSION_PLACEHOLDER);

        let prefix = parts.next().unwrap_or_default().to_owned();
        let Some(suffix) = parts.next() else {
            return Err(TagPatternError::NoVersion {
                pattern: pattern.to_owned(),
            });
        };

        if parts.next().is_some() {
            return Err(TagPatternError::RepeatedVersion {
                pattern: pattern.to_owned(),
            });
        }

        Ok(Self {
            prefix,
            suffix: suffix.to_owned(),
        })
    }

    /// The tag naming `version`.
    #[must_use]
    pub fn render(&self, version: &Version) -> String {
        format!("{}{version}{}", self.prefix, self.suffix)
    }

    /// The version a tag claims, if the tag was named by this pattern.
    ///
    /// Returns `None` when the tag does not fit the pattern, or fits it but
    /// carries something that is not a version.
    #[must_use]
    pub fn extract(&self, tag: &str) -> Option<Version> {
        let rest = tag.strip_prefix(&self.prefix)?;
        let middle = rest.strip_suffix(&self.suffix)?;

        // An empty middle means the tag is exactly the prefix and suffix, with
        // no version between them.
        if middle.is_empty() {
            return None;
        }

        middle.parse().ok()
    }
}

/// Substitutes the project name into a template.
///
/// A pattern carrying no project placeholder is returned unchanged. Returns
/// `None` when the pattern names a project but there is none to name, which
/// happens only in a repository whose single project is unnamed.
#[must_use]
pub fn apply_project_name(pattern: &str, project: Option<&str>) -> Option<String> {
    if !pattern.contains(PROJECT_PLACEHOLDER) {
        return Some(pattern.to_owned());
    }

    project.map(|name| pattern.replace(PROJECT_PLACEHOLDER, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        text.parse().unwrap()
    }

    #[test]
    fn renders_a_tag_from_a_version() {
        let p = TagPattern::parse("v{new_version}").unwrap();
        assert_eq!(p.render(&v("1.2.3")), "v1.2.3");

        let p = TagPattern::parse("api-v{new_version}").unwrap();
        assert_eq!(p.render(&v("0.4.0-rc.1")), "api-v0.4.0-rc.1");
    }

    #[test]
    fn reads_a_version_back_out_of_a_tag() {
        let p = TagPattern::parse("api-v{new_version}").unwrap();
        assert_eq!(p.extract("api-v1.2.3"), Some(v("1.2.3")));
        assert_eq!(p.extract("api-v1.2.3-rc.1"), Some(v("1.2.3-rc.1")));
    }

    #[test]
    fn rendering_and_reading_are_inverses() {
        let patterns = ["v{new_version}", "api-v{new_version}", "{new_version}"];
        let versions = ["1.2.3", "0.1.0-alpha.0", "10.20.30-rc.4"];

        for pattern in patterns {
            let p = TagPattern::parse(pattern).unwrap();
            for version in versions {
                let version = v(version);
                assert_eq!(
                    p.extract(&p.render(&version)),
                    Some(version.clone()),
                    "{pattern} / {version}"
                );
            }
        }
    }

    #[test]
    fn a_tag_from_another_pattern_is_not_matched() {
        let api = TagPattern::parse("api-v{new_version}").unwrap();
        assert_eq!(api.extract("web-v1.2.3"), None);
        assert_eq!(api.extract("v1.2.3"), None);
    }

    #[test]
    fn a_prefix_that_is_not_followed_by_a_version_is_not_matched() {
        let p = TagPattern::parse("api-v{new_version}").unwrap();
        assert_eq!(p.extract("api-v"), None);
        assert_eq!(p.extract("api-vlatest"), None);
        assert_eq!(p.extract("api-vnightly.1"), None);
    }

    #[test]
    fn a_suffix_is_honored() {
        let p = TagPattern::parse("release/{new_version}/final").unwrap();
        assert_eq!(p.render(&v("2.0.0")), "release/2.0.0/final");
        assert_eq!(p.extract("release/2.0.0/final"), Some(v("2.0.0")));
        // Missing the suffix means the tag was not named by this pattern.
        assert_eq!(p.extract("release/2.0.0"), None);
    }

    #[test]
    fn a_pattern_must_place_the_version_exactly_once() {
        assert!(matches!(
            TagPattern::parse("release").unwrap_err(),
            TagPatternError::NoVersion { .. }
        ));
        assert!(matches!(
            TagPattern::parse("v{new_version}-{new_version}").unwrap_err(),
            TagPatternError::RepeatedVersion { .. }
        ));
    }

    #[test]
    fn similar_prefixes_do_not_collide() {
        // "api" is a prefix of "api-extra"; neither pattern may claim the
        // other's tags.
        let api = TagPattern::parse("api-v{new_version}").unwrap();
        let extra = TagPattern::parse("api-extra-v{new_version}").unwrap();

        assert_eq!(extra.extract("api-extra-v1.0.0"), Some(v("1.0.0")));
        assert_eq!(api.extract("api-extra-v1.0.0"), None);
    }

    #[test]
    fn the_project_name_is_substituted() {
        assert_eq!(
            apply_project_name("{project}-v{new_version}", Some("api")).unwrap(),
            "api-v{new_version}"
        );
        // A pattern without the placeholder is returned untouched.
        assert_eq!(
            apply_project_name("v{new_version}", None).unwrap(),
            "v{new_version}"
        );
        // Naming a project that has no name cannot be satisfied.
        assert!(apply_project_name("{project}-v{new_version}", None).is_none());
    }
}
