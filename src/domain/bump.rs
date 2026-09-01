//! The version transition state machine.
//!
//! A version is either *stable* (`1.2.3`) or a *pre-release* (`1.2.3-beta.4`).
//! [`apply`] is the single entry point that maps a current version and a
//! requested [`Transition`] to the resulting version, or to an error
//! explaining why the transition is not meaningful.

use std::fmt;
use std::str::FromStr;

use semver::{Prerelease, Version};
use thiserror::Error;

/// A bump of a stable version's numeric components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StableBump {
    /// `1.2.3` -> `1.2.4`
    Patch,
    /// `1.2.3` -> `1.3.0`
    Minor,
    /// `1.2.3` -> `2.0.0`
    Major,
}

impl fmt::Display for StableBump {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Patch => "patch",
            Self::Minor => "minor",
            Self::Major => "major",
        })
    }
}

impl FromStr for StableBump {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "patch" => Ok(Self::Patch),
            "minor" => Ok(Self::Minor),
            "major" => Ok(Self::Major),
            other => Err(TransitionError::UnknownStableBump(other.to_owned())),
        }
    }
}

/// A pre-release channel.
///
/// The derived ordering is the maturity ordering (`alpha < beta < rc`) and is
/// relied upon to reject moves toward a less mature channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PreLabel {
    /// Earliest channel.
    Alpha,
    /// Intermediate channel.
    Beta,
    /// Release candidate; the most mature pre-release channel.
    Rc,
}

impl fmt::Display for PreLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Alpha => "alpha",
            Self::Beta => "beta",
            Self::Rc => "rc",
        })
    }
}

impl FromStr for PreLabel {
    type Err = TransitionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "alpha" => Ok(Self::Alpha),
            "beta" => Ok(Self::Beta),
            "rc" => Ok(Self::Rc),
            other => Err(TransitionError::UnknownPreLabel(other.to_owned())),
        }
    }
}

/// A requested version transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// Bump a stable version's numeric components.
    Stable(StableBump),
    /// Start or advance a pre-release.
    ///
    /// `from` is required when the current version is stable, because starting
    /// a pre-release means choosing which future stable version it precedes.
    /// It is ignored when already on a pre-release.
    PreRelease {
        /// Channel to move to.
        label: PreLabel,
        /// Stable bump the new pre-release is based on.
        from: Option<StableBump>,
    },
    /// Drop the pre-release suffix, finalizing the version.
    Release,
}

/// Why a requested transition cannot be performed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransitionError {
    /// `release` was requested on a version that is already stable.
    #[error("{current} is already a stable version; there is nothing to release")]
    AlreadyStable {
        /// The version the transition was attempted from.
        current: Version,
    },

    /// A stable bump was requested while on a pre-release.
    ///
    /// This is ambiguous — it could mean "finalize this version" or "abandon
    /// it and move on" — so it is rejected in favor of an explicit `release`.
    #[error(
        "{current} is a pre-release; run `vump release` to finalize it as {finalized}, \
         then bump from there"
    )]
    StableBumpOnPreRelease {
        /// The version the transition was attempted from.
        current: Version,
        /// The stable version `release` would produce.
        finalized: Version,
    },

    /// Starting a pre-release from a stable version without saying which
    /// stable version it precedes.
    #[error(
        "{current} is stable, so `{label}` needs to know which release it leads to; \
         pass --from patch, --from minor, or --from major"
    )]
    MissingFrom {
        /// The version the transition was attempted from.
        current: Version,
        /// The requested pre-release channel.
        label: PreLabel,
    },

    /// A move toward a less mature pre-release channel.
    #[error("{current} cannot move backwards from {from} to {to}")]
    BackwardsPreRelease {
        /// The version the transition was attempted from.
        current: Version,
        /// The current channel.
        from: PreLabel,
        /// The requested, less mature channel.
        to: PreLabel,
    },

    /// The current version carries a pre-release that vump does not model.
    #[error("{current} has an unrecognized pre-release {pre:?}; expected alpha, beta, or rc")]
    UnrecognizedPreRelease {
        /// The version the transition was attempted from.
        current: Version,
        /// The pre-release text that could not be interpreted.
        pre: String,
    },

    /// A pre-release channel name outside the supported set.
    #[error("unknown pre-release channel {0:?}; expected alpha, beta, or rc")]
    UnknownPreLabel(String),

    /// A stable bump name outside the supported set.
    #[error("unknown bump {0:?}; expected patch, minor, or major")]
    UnknownStableBump(String),
}

/// Applies `transition` to `current`, returning the resulting version.
///
/// # Errors
///
/// Returns a [`TransitionError`] when the transition is not meaningful from
/// the current version — see that type's variants for the specific cases.
pub fn apply(current: &Version, transition: Transition) -> Result<Version, TransitionError> {
    let state = PreState::read(current)?;

    match (transition, state) {
        (Transition::Stable(bump), PreState::Stable) => Ok(bump_stable(current, bump)),

        (Transition::Stable(_), PreState::Pre { .. }) => {
            Err(TransitionError::StableBumpOnPreRelease {
                current: current.clone(),
                finalized: finalize(current),
            })
        }

        (Transition::PreRelease { label, from }, PreState::Stable) => {
            let Some(from) = from else {
                return Err(TransitionError::MissingFrom {
                    current: current.clone(),
                    label,
                });
            };
            Ok(with_pre(&bump_stable(current, from), label, 0))
        }

        (
            Transition::PreRelease { label, .. },
            PreState::Pre {
                label: current_label,
                number,
            },
        ) => match label.cmp(&current_label) {
            std::cmp::Ordering::Less => Err(TransitionError::BackwardsPreRelease {
                current: current.clone(),
                from: current_label,
                to: label,
            }),
            // Same channel continues its sequence; a more mature channel starts
            // a new one.
            std::cmp::Ordering::Equal => Ok(with_pre(current, label, number + 1)),
            std::cmp::Ordering::Greater => Ok(with_pre(current, label, 0)),
        },

        (Transition::Release, PreState::Pre { .. }) => Ok(finalize(current)),

        (Transition::Release, PreState::Stable) => Err(TransitionError::AlreadyStable {
            current: current.clone(),
        }),
    }
}

/// Every transition that is meaningful from `current`, with its result.
///
/// This is what an interactive menu offers: only operations that would succeed
/// are listed. Showing an impossible choice annotated with why it will fail
/// asks the reader to do work the tool has already done.
///
/// Ordering is by increasing disruption — the ordinary bumps first, then
/// pre-releases, then finalizing — so the common choice is nearest the top.
///
/// # Errors
///
/// Returns a [`TransitionError`] when `current` itself cannot be interpreted,
/// which is the one case where no transition is offerable.
pub fn valid_transitions(current: &Version) -> Result<Vec<(Transition, Version)>, TransitionError> {
    // Rejects an unrecognized pre-release up front rather than silently
    // returning an empty menu.
    let state = PreState::read(current)?;

    let mut candidates = Vec::new();

    match state {
        PreState::Stable => {
            for bump in [StableBump::Patch, StableBump::Minor, StableBump::Major] {
                candidates.push(Transition::Stable(bump));
            }
            // A pre-release from a stable version must name the release it
            // precedes, so each channel is offered once per base.
            for label in [PreLabel::Alpha, PreLabel::Beta, PreLabel::Rc] {
                for bump in [StableBump::Patch, StableBump::Minor, StableBump::Major] {
                    candidates.push(Transition::PreRelease {
                        label,
                        from: Some(bump),
                    });
                }
            }
        }
        PreState::Pre { .. } => {
            for label in [PreLabel::Alpha, PreLabel::Beta, PreLabel::Rc] {
                candidates.push(Transition::PreRelease { label, from: None });
            }
            candidates.push(Transition::Release);
        }
    }

    Ok(candidates
        .into_iter()
        .filter_map(|t| apply(current, t).ok().map(|next| (t, next)))
        .collect())
}

/// Whether a version is stable, and if not, how its pre-release reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreState {
    Stable,
    Pre { label: PreLabel, number: u64 },
}

impl PreState {
    /// Interprets the pre-release portion of `version`.
    ///
    /// A bare channel with no counter (`1.2.3-beta`) is read as counter zero,
    /// so that advancing it yields `1.2.3-beta.1`.
    fn read(version: &Version) -> Result<Self, TransitionError> {
        if version.pre.is_empty() {
            return Ok(Self::Stable);
        }

        let text = version.pre.as_str();
        let (label_text, number) = match text.split_once('.') {
            Some((label, rest)) => {
                let number =
                    rest.parse::<u64>()
                        .map_err(|_| TransitionError::UnrecognizedPreRelease {
                            current: version.clone(),
                            pre: text.to_owned(),
                        })?;
                (label, number)
            }
            None => (text, 0),
        };

        let label = label_text.parse::<PreLabel>().map_err(|_| {
            TransitionError::UnrecognizedPreRelease {
                current: version.clone(),
                pre: text.to_owned(),
            }
        })?;

        Ok(Self::Pre { label, number })
    }
}

/// Increments the numeric components of a version, discarding any pre-release
/// and build metadata.
fn bump_stable(version: &Version, bump: StableBump) -> Version {
    match bump {
        StableBump::Patch => Version::new(version.major, version.minor, version.patch + 1),
        StableBump::Minor => Version::new(version.major, version.minor + 1, 0),
        StableBump::Major => Version::new(version.major + 1, 0, 0),
    }
}

/// Strips the pre-release and build metadata, keeping the numeric components.
fn finalize(version: &Version) -> Version {
    Version::new(version.major, version.minor, version.patch)
}

/// Returns `version`'s numeric components carrying the pre-release
/// `label.number`.
fn with_pre(version: &Version, label: PreLabel, number: u64) -> Version {
    let mut next = Version::new(version.major, version.minor, version.patch);
    // `label` is a closed set of identifiers and `number` is decimal digits, so
    // the composed string is always a syntactically valid pre-release.
    next.pre = Prerelease::new(&format!("{label}.{number}"))
        .expect("composed pre-release is always valid");
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> Version {
        text.parse().expect("test version literal must be valid")
    }

    /// Asserts the full transition table from the design specification.
    #[test]
    fn accepted_transitions() {
        let cases: &[(&str, Transition, &str)] = &[
            ("1.2.3", Transition::Stable(StableBump::Patch), "1.2.4"),
            ("1.2.3", Transition::Stable(StableBump::Minor), "1.3.0"),
            ("1.2.3", Transition::Stable(StableBump::Major), "2.0.0"),
            (
                "1.2.3",
                Transition::PreRelease {
                    label: PreLabel::Alpha,
                    from: Some(StableBump::Patch),
                },
                "1.2.4-alpha.0",
            ),
            (
                "1.2.3",
                Transition::PreRelease {
                    label: PreLabel::Alpha,
                    from: Some(StableBump::Minor),
                },
                "1.3.0-alpha.0",
            ),
            (
                "1.2.3",
                Transition::PreRelease {
                    label: PreLabel::Rc,
                    from: Some(StableBump::Major),
                },
                "2.0.0-rc.0",
            ),
            (
                "1.2.3-alpha.0",
                Transition::PreRelease {
                    label: PreLabel::Alpha,
                    from: None,
                },
                "1.2.3-alpha.1",
            ),
            (
                "1.2.3-alpha.2",
                Transition::PreRelease {
                    label: PreLabel::Beta,
                    from: None,
                },
                "1.2.3-beta.0",
            ),
            (
                "1.2.3-beta.1",
                Transition::PreRelease {
                    label: PreLabel::Rc,
                    from: None,
                },
                "1.2.3-rc.0",
            ),
            (
                "1.2.3-alpha.5",
                Transition::PreRelease {
                    label: PreLabel::Rc,
                    from: None,
                },
                "1.2.3-rc.0",
            ),
            ("1.2.3-rc.1", Transition::Release, "1.2.3"),
            ("1.2.3-alpha.0", Transition::Release, "1.2.3"),
        ];

        for (current, transition, expected) in cases {
            let result = apply(&v(current), *transition)
                .unwrap_or_else(|e| panic!("{current} + {transition:?} should succeed: {e}"));
            assert_eq!(result, v(expected), "{current} + {transition:?}");
        }
    }

    #[test]
    fn release_on_stable_is_rejected() {
        let err = apply(&v("1.2.3"), Transition::Release).unwrap_err();
        assert!(matches!(err, TransitionError::AlreadyStable { .. }));
    }

    #[test]
    fn stable_bump_on_pre_release_is_rejected() {
        for bump in [StableBump::Patch, StableBump::Minor, StableBump::Major] {
            let err = apply(&v("1.2.3-rc.1"), Transition::Stable(bump)).unwrap_err();
            let TransitionError::StableBumpOnPreRelease { finalized, .. } = err else {
                panic!("expected StableBumpOnPreRelease for {bump}, got {err:?}");
            };
            // The error must name the version `release` would produce, since
            // that is the action it directs the user toward.
            assert_eq!(finalized, v("1.2.3"));
        }
    }

    #[test]
    fn pre_release_from_stable_requires_a_base() {
        let err = apply(
            &v("1.2.3"),
            Transition::PreRelease {
                label: PreLabel::Beta,
                from: None,
            },
        )
        .unwrap_err();
        assert!(matches!(err, TransitionError::MissingFrom { .. }));
    }

    #[test]
    fn moving_to_a_less_mature_channel_is_rejected() {
        let cases = [
            ("1.2.3-beta.0", PreLabel::Alpha),
            ("1.2.3-rc.0", PreLabel::Alpha),
            ("1.2.3-rc.3", PreLabel::Beta),
        ];
        for (current, label) in cases {
            let err = apply(&v(current), Transition::PreRelease { label, from: None }).unwrap_err();
            assert!(
                matches!(err, TransitionError::BackwardsPreRelease { .. }),
                "{current} -> {label} should be refused, got {err:?}"
            );
        }
    }

    #[test]
    fn from_is_ignored_when_already_on_a_pre_release() {
        // The current pre-release already fixes the numeric components; a
        // stale --from must not silently re-bump them.
        let result = apply(
            &v("1.2.3-alpha.0"),
            Transition::PreRelease {
                label: PreLabel::Beta,
                from: Some(StableBump::Major),
            },
        )
        .expect("advancing a pre-release should succeed");
        assert_eq!(result, v("1.2.3-beta.0"));
    }

    #[test]
    fn bare_channel_without_a_counter_is_read_as_zero() {
        let result = apply(
            &v("1.2.3-beta"),
            Transition::PreRelease {
                label: PreLabel::Beta,
                from: None,
            },
        )
        .expect("advancing a counterless pre-release should succeed");
        assert_eq!(result, v("1.2.3-beta.1"));
    }

    #[test]
    fn unrecognized_pre_release_is_reported() {
        let err = apply(&v("1.2.3-nightly.4"), Transition::Release).unwrap_err();
        assert!(matches!(
            err,
            TransitionError::UnrecognizedPreRelease { .. }
        ));
    }

    #[test]
    fn build_metadata_is_discarded() {
        let result = apply(&v("1.2.3+build.99"), Transition::Stable(StableBump::Patch))
            .expect("bumping a version with build metadata should succeed");
        assert_eq!(result, v("1.2.4"));
        assert!(result.build.is_empty());
    }

    #[test]
    fn a_stable_version_offers_bumps_and_every_pre_release_base() {
        let offered = valid_transitions(&v("1.2.3")).unwrap();
        let results: Vec<String> = offered.iter().map(|(_, r)| r.to_string()).collect();

        assert_eq!(
            results,
            [
                "1.2.4",
                "1.3.0",
                "2.0.0",
                "1.2.4-alpha.0",
                "1.3.0-alpha.0",
                "2.0.0-alpha.0",
                "1.2.4-beta.0",
                "1.3.0-beta.0",
                "2.0.0-beta.0",
                "1.2.4-rc.0",
                "1.3.0-rc.0",
                "2.0.0-rc.0",
            ]
        );
    }

    #[test]
    fn a_stable_version_is_not_offered_release() {
        let offered = valid_transitions(&v("1.2.3")).unwrap();
        assert!(
            !offered.iter().any(|(t, _)| *t == Transition::Release),
            "there is nothing to finalize from a stable version"
        );
    }

    #[test]
    fn a_pre_release_offers_only_forward_moves_and_finalizing() {
        let offered = valid_transitions(&v("1.3.0-beta.1")).unwrap();
        let results: Vec<String> = offered.iter().map(|(_, r)| r.to_string()).collect();

        // alpha is absent: it would be a move backwards.
        assert_eq!(results, ["1.3.0-beta.2", "1.3.0-rc.0", "1.3.0"]);
    }

    #[test]
    fn every_offered_transition_actually_succeeds() {
        for current in ["0.1.0", "1.2.3", "1.2.3-alpha.0", "2.0.0-rc.7"] {
            let version = v(current);
            for (transition, expected) in valid_transitions(&version).unwrap() {
                let actual = apply(&version, transition).unwrap_or_else(|e| {
                    panic!("{current} offered {transition:?} but it failed: {e}")
                });
                assert_eq!(actual, expected, "{current} + {transition:?}");
            }
        }
    }

    #[test]
    fn an_uninterpretable_version_offers_nothing() {
        assert!(valid_transitions(&v("1.2.3-nightly.4")).is_err());
    }

    #[test]
    fn channels_are_ordered_by_maturity() {
        assert!(PreLabel::Alpha < PreLabel::Beta);
        assert!(PreLabel::Beta < PreLabel::Rc);
    }

    #[test]
    fn labels_round_trip_through_text() {
        for label in [PreLabel::Alpha, PreLabel::Beta, PreLabel::Rc] {
            assert_eq!(label.to_string().parse::<PreLabel>(), Ok(label));
        }
        for bump in [StableBump::Patch, StableBump::Minor, StableBump::Major] {
            assert_eq!(bump.to_string().parse::<StableBump>(), Ok(bump));
        }
    }
}
