package semver

import (
	"fmt"
	"strconv"

	"github.com/blang/semver/v4"
)

// BumpType is the operation the user wants to perform.
type BumpType string

const (
	BumpPatch   BumpType = "patch"
	BumpMinor   BumpType = "minor"
	BumpMajor   BumpType = "major"
	BumpAlpha   BumpType = "alpha"
	BumpBeta    BumpType = "beta"
	BumpRC      BumpType = "rc"
	BumpRelease BumpType = "release"
)

// preReleaseOrder defines the canonical ordering of pre-release labels.
var preReleaseOrder = map[string]int{
	"alpha": 0,
	"beta":  1,
	"rc":    2,
}

// ParseVersion parses a semver string, returning a blang semver.Version.
func ParseVersion(v string) (semver.Version, error) {
	parsed, err := semver.ParseTolerant(v)
	if err != nil {
		return semver.Version{}, fmt.Errorf("invalid semver %q: %w", v, err)
	}
	return parsed, nil
}

// IsStable returns true if the version has no pre-release identifiers.
func IsStable(v semver.Version) bool {
	return len(v.Pre) == 0
}

// PreLabel returns the pre-release label string (e.g. "alpha", "beta", "rc")
// or "" if the version is stable.
func PreLabel(v semver.Version) string {
	if len(v.Pre) == 0 {
		return ""
	}
	return v.Pre[0].VersionStr
}

// PreNum returns the pre-release number (e.g. 2 for "alpha.2") or 0 if none.
func PreNum(v semver.Version) uint64 {
	if len(v.Pre) < 2 {
		return 0
	}
	return v.Pre[1].VersionNum
}

// BumpResult contains the computed new version and a description of what happened.
type BumpResult struct {
	New semver.Version
}

// BumpStable computes the result of patch/minor/major on a stable version.
func BumpStable(v semver.Version, bump BumpType) (semver.Version, error) {
	if !IsStable(v) {
		return semver.Version{}, fmt.Errorf("BumpStable called on pre-release version %s", v)
	}
	result := v
	switch bump {
	case BumpPatch:
		result.Patch++
	case BumpMinor:
		result.Minor++
		result.Patch = 0
	case BumpMajor:
		result.Major++
		result.Minor = 0
		result.Patch = 0
	default:
		return semver.Version{}, fmt.Errorf("BumpStable: unexpected bump type %q", bump)
	}
	result.Pre = nil
	result.Build = nil
	return result, nil
}

// StartPreRelease bumps the stable base (by baseBump: patch/minor/major) and
// appends "-label.0".  baseBump must be patch, minor, or major.
func StartPreRelease(v semver.Version, baseBump BumpType, label string) (semver.Version, error) {
	base, err := BumpStable(v, baseBump)
	if err != nil {
		return semver.Version{}, err
	}
	return appendPreRelease(base, label, 0), nil
}

// BumpPreRelease computes the result of advancing a pre-release version.
// Handles: same label (increment num), higher label (reset to .0), lower label (error unless forced).
func BumpPreRelease(v semver.Version, targetLabel string, force bool) (semver.Version, error) {
	currentLabel := PreLabel(v)
	currentNum := PreNum(v)

	currentRank, currentKnown := preReleaseOrder[currentLabel]
	targetRank, targetKnown := preReleaseOrder[targetLabel]

	if !currentKnown {
		return semver.Version{}, fmt.Errorf("unknown pre-release label %q in current version", currentLabel)
	}
	if !targetKnown {
		return semver.Version{}, fmt.Errorf("unknown pre-release label %q", targetLabel)
	}

	if targetRank < currentRank && !force {
		return semver.Version{}, &BackwardsError{
			CurrentLabel: currentLabel,
			TargetLabel:  targetLabel,
		}
	}

	base := semver.Version{
		Major: v.Major,
		Minor: v.Minor,
		Patch: v.Patch,
	}

	switch {
	case targetRank == currentRank:
		// Same label → increment number.
		return appendPreRelease(base, targetLabel, currentNum+1), nil
	default:
		// Different label → reset to .0.
		return appendPreRelease(base, targetLabel, 0), nil
	}
}

// DropPreRelease returns the stable version corresponding to v (strips pre-release).
func DropPreRelease(v semver.Version) semver.Version {
	return semver.Version{
		Major: v.Major,
		Minor: v.Minor,
		Patch: v.Patch,
	}
}

// BackwardsError is returned when the user tries to go to a lower pre-release label.
type BackwardsError struct {
	CurrentLabel string
	TargetLabel  string
}

func (e *BackwardsError) Error() string {
	return fmt.Sprintf(
		"going backwards (%s → %s) is not allowed — use --force to override",
		e.CurrentLabel,
		e.TargetLabel,
	)
}

// IsBackwardsError returns true if err is a BackwardsError.
func IsBackwardsError(err error) bool {
	_, ok := err.(*BackwardsError)
	return ok
}

// appendPreRelease returns v with pre-release set to "label.num".
func appendPreRelease(v semver.Version, label string, num uint64) semver.Version {
	v.Pre = []semver.PRVersion{
		{VersionStr: label, IsNum: false},
		{VersionNum: num, IsNum: true},
	}
	v.Build = nil
	return v
}

// LabelFromBumpType extracts the pre-release label string from a BumpType.
// Returns "", false if the bump type is not a pre-release bump.
func LabelFromBumpType(bt BumpType) (string, bool) {
	switch bt {
	case BumpAlpha:
		return "alpha", true
	case BumpBeta:
		return "beta", true
	case BumpRC:
		return "rc", true
	}
	return "", false
}

// ValidFromBase validates that --from value is one of patch/minor/major.
func ValidFromBase(from string) (BumpType, error) {
	switch BumpType(from) {
	case BumpPatch, BumpMinor, BumpMajor:
		return BumpType(from), nil
	}
	return "", fmt.Errorf("--from must be patch, minor, or major (got %q)", from)
}

// FormatPreOptions returns a slice of "(baseBump) → result" strings for the
// interactive "start pre-release" prompt, given a stable current version.
func FormatPreOptions(v semver.Version, label string) []string {
	options := []string{}
	for _, base := range []BumpType{BumpPatch, BumpMinor, BumpMajor} {
		result, err := StartPreRelease(v, base, label)
		if err != nil {
			continue
		}
		options = append(options, fmt.Sprintf("%s  (%s bump)", result.String(), string(base)))
	}
	return options
}

// FormatPreOptionVersions returns the computed versions (not display strings)
// for patch/minor/major pre-release start options.
func FormatPreOptionVersions(v semver.Version, label string) []semver.Version {
	var versions []semver.Version
	for _, base := range []BumpType{BumpPatch, BumpMinor, BumpMajor} {
		result, err := StartPreRelease(v, base, label)
		if err != nil {
			continue
		}
		versions = append(versions, result)
	}
	return versions
}

// FormatStableReleaseOptions returns the two options when patch/minor/major
// is requested on a pre-release: "Release X.Y.Z" or "Bump to X.Y.Z+n"
func FormatStableReleaseOptions(v semver.Version, bump BumpType) (release semver.Version, bumped semver.Version, err error) {
	release = DropPreRelease(v)
	stable := DropPreRelease(v)
	bumped, err = BumpStable(stable, bump)
	return
}

// PreNumString converts a uint64 to a string — helper for display.
func PreNumString(n uint64) string {
	return strconv.FormatUint(n, 10)
}
