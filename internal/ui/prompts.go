package ui

import (
	"fmt"
	"path/filepath"

	bsemver "github.com/blang/semver/v4"
	"github.com/charmbracelet/huh"
	"github.com/okcodes/vump/internal/semver"
)

// VersionedFile pairs a file path with its currently-read version.
type VersionedFile struct {
	Path    string
	Version bsemver.Version
}

// SelectBaseVersion asks the user to choose the base version when files are out of sync.
// It groups files by version string for clean display.
func SelectBaseVersion(files []VersionedFile) (bsemver.Version, error) {
	// Group paths by version string.
	type group struct {
		version bsemver.Version
		paths   []string
	}
	seen := map[string]*group{}
	var order []string
	for _, f := range files {
		vs := f.Version.String()
		if _, ok := seen[vs]; !ok {
			seen[vs] = &group{version: f.Version}
			order = append(order, vs)
		}
		seen[vs].paths = append(seen[vs].paths, f.Path)
	}

	var options []huh.Option[string]
	for _, vs := range order {
		g := seen[vs]
		label := fmt.Sprintf("%-20s (%s)", vs, joinBasenames(g.paths))
		options = append(options, huh.NewOption(label, vs))
	}

	var selected string
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title("⚠  Version mismatch detected across files.\n\n  Which version should we use as the base for this bump?").
				Options(options...).
				Value(&selected),
		),
	)
	if err := form.Run(); err != nil {
		return bsemver.Version{}, err
	}
	return seen[selected].version, nil
}

// SelectBumpType asks the user interactively what kind of bump to perform.
// It shows all bump types with computed results and flags backwards moves.
func SelectBumpType(current bsemver.Version) (semver.BumpType, error) {
	type option struct {
		label   string
		bump    semver.BumpType
		display string
	}

	isStable := semver.IsStable(current)
	curLabel := semver.PreLabel(current)

	var opts []huh.Option[string]

	buildOpt := func(bt semver.BumpType, result string, note string) huh.Option[string] {
		label := fmt.Sprintf("%-10s →  %s", string(bt), result)
		if note != "" {
			label += "  " + note
		}
		return huh.NewOption(label, string(bt))
	}

	if isStable {
		// Stable: patch/minor/major straight bumps.
		for _, bt := range []semver.BumpType{semver.BumpPatch, semver.BumpMinor, semver.BumpMajor} {
			result, _ := semver.BumpStable(current, bt)
			opts = append(opts, buildOpt(bt, result.String(), ""))
		}
		// Pre-release options: inform the user they'll be asked for a base.
		for _, bt := range []semver.BumpType{semver.BumpAlpha, semver.BumpBeta, semver.BumpRC} {
			opts = append(opts, buildOpt(bt, "(you'll be asked what base to use)", ""))
		}
		// release is an error from stable but show it greyed out by using a note.
		opts = append(opts, buildOpt(semver.BumpRelease, "already stable, nothing to release", ""))
	} else {
		// Pre-release: show what each bump would do.
		preRankMap := map[string]int{"alpha": 0, "beta": 1, "rc": 2}
		curRank := preRankMap[curLabel]

		for _, bt := range []semver.BumpType{semver.BumpPatch, semver.BumpMinor, semver.BumpMajor} {
			opts = append(opts, buildOpt(bt, "(you'll be asked: release or bump further?)", ""))
		}

		for _, bt := range []semver.BumpType{semver.BumpAlpha, semver.BumpBeta, semver.BumpRC} {
			label, _ := semver.LabelFromBumpType(bt)
			rank := preRankMap[label]
			if rank < curRank {
				opts = append(opts, buildOpt(bt, "(would go backwards — requires --force)", ""))
			} else {
				var result bsemver.Version
				if label == curLabel {
					result, _ = semver.BumpPreRelease(current, label, false)
				} else {
					result, _ = semver.BumpPreRelease(current, label, false)
				}
				opts = append(opts, buildOpt(bt, result.String(), ""))
			}
		}
		// release
		released := semver.DropPreRelease(current)
		opts = append(opts, buildOpt(semver.BumpRelease, released.String(), ""))
	}

	var selected string
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title(fmt.Sprintf("Current version: %s\n\n  Bump type:", current.String())).
				Options(opts...).
				Value(&selected),
		),
	)
	if err := form.Run(); err != nil {
		return "", err
	}
	return semver.BumpType(selected), nil
}

// SelectPreReleaseBase asks the user which base bump (patch/minor/major) to use
// when starting a pre-release from a stable version.
func SelectPreReleaseBase(current bsemver.Version, label string) (bsemver.Version, error) {
	type entry struct {
		version bsemver.Version
		base    semver.BumpType
	}
	bases := []semver.BumpType{semver.BumpPatch, semver.BumpMinor, semver.BumpMajor}
	var entries []entry
	var opts []huh.Option[string]
	for _, base := range bases {
		result, err := semver.StartPreRelease(current, base, label)
		if err != nil {
			continue
		}
		entries = append(entries, entry{result, base})
		opts = append(opts, huh.NewOption(
			fmt.Sprintf("%-20s (%s bump)", result.String(), string(base)),
			result.String(),
		))
	}

	var selected string
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title(fmt.Sprintf(
					"You're on a stable version (%s). Starting a pre-release requires bumping\nthe version number first. What should the pre-release base be?",
					current.String(),
				)).
				Options(opts...).
				Value(&selected),
		),
	)
	if err := form.Run(); err != nil {
		return bsemver.Version{}, err
	}
	for _, e := range entries {
		if e.version.String() == selected {
			return e.version, nil
		}
	}
	return bsemver.Version{}, fmt.Errorf("no result selected")
}

// SelectReleaseBehavior asks whether to finalize the pre-release as stable
// or bump the stable version further, when patch/minor/major is requested on
// a pre-release version.
func SelectReleaseBehavior(current bsemver.Version, bump semver.BumpType) (bsemver.Version, error) {
	release, bumped, err := semver.FormatStableReleaseOptions(current, bump)
	if err != nil {
		return bsemver.Version{}, err
	}

	relLabel := fmt.Sprintf("Release %s  (finalize this version, drop the pre-release suffix)", release.String())
	bumpLabel := fmt.Sprintf("Bump to %s  (skip %s stable and go straight to next %s)", bumped.String(), release.String(), string(bump))

	var selected string
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title(fmt.Sprintf("You're on %s (a pre-release). What do you want to do?", current.String())).
				Options(
					huh.NewOption(relLabel, release.String()),
					huh.NewOption(bumpLabel, bumped.String()),
				).
				Value(&selected),
		),
	)
	if err := form.Run(); err != nil {
		return bsemver.Version{}, err
	}
	parsed, err := bsemver.ParseTolerant(selected)
	if err != nil {
		return bsemver.Version{}, err
	}
	return parsed, nil
}

// GitAction represents the git operation to perform after a version bump.
type GitAction int

const (
	GitActionNone   GitAction = iota // just write the files
	GitActionCommit                  // git add + commit
	GitActionTag                     // git add + commit + tag
)

// SelectGitAction asks the user what git operation to run after bumping.
// defaultAction pre-selects the option that matches the user's vump.toml config.
func SelectGitAction(defaultAction GitAction) (GitAction, error) {
	type opt struct {
		label  string
		action GitAction
	}
	const (
		valNone   = "none"
		valCommit = "commit"
		valTag    = "tag"
	)
	defaultVal := valNone
	switch defaultAction {
	case GitActionCommit:
		defaultVal = valCommit
	case GitActionTag:
		defaultVal = valTag
	}

	var selected string
	selected = defaultVal // pre-select based on config
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewSelect[string]().
				Title("Git action after bumping:").
				Options(
					huh.NewOption("None    — just write the files", valNone),
					huh.NewOption("Commit  — git add + commit", valCommit),
					huh.NewOption("Tag     — git add + commit + tag  (tag implies commit)", valTag),
				).
				Value(&selected),
		),
	)
	if err := form.Run(); err != nil {
		return GitActionNone, err
	}
	switch selected {
	case valCommit:
		return GitActionCommit, nil
	case valTag:
		return GitActionTag, nil
	default:
		return GitActionNone, nil
	}
}

// Confirm asks the user to approve the bump. Shows the git action in the summary.
func Confirm(from, to bsemver.Version, gitAction GitAction, tagPattern string, doPush bool) (bool, error) {
	gitLine := ""
	switch gitAction {
	case GitActionCommit:
		gitLine = "\nGit:     commit"
	case GitActionTag:
		gitLine = fmt.Sprintf("\nGit:     tag %s", tagPattern)
	}
	if gitAction != GitActionNone {
		if doPush {
			gitLine += "\nPush:    Yes"
		} else {
			gitLine += "\nPush:    No"
		}
	}

	var confirmed bool
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewConfirm().
				Title(fmt.Sprintf("Bumping: %s  →  %s%s", from.String(), to.String(), gitLine)).
				Affirmative("Yes").
				Negative("No").
				Value(&confirmed),
		),
	)
	if err := form.Run(); err != nil {
		return false, err
	}
	return confirmed, nil
}

// ConfirmPush asks whether to push the commit (and tag) to the remote.
// configDefault pre-selects Yes if push = true is set in vump.toml.
func ConfirmPush(configDefault bool) (bool, error) {
	doPush := configDefault // honoured as the initial selection
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewConfirm().
				Title("Push to remote after committing?").
				Affirmative("Yes").
				Negative("No").
				Value(&doPush),
		),
	)
	if err := form.Run(); err != nil {
		return false, err
	}
	return doPush, nil
}

// joinBasenames produces "a, b, c" from a list of paths using only base filenames.
func joinBasenames(paths []string) string {
	result := ""
	for i, p := range paths {
		if i > 0 {
			result += ", "
		}
		result += filepath.Base(p)
	}
	return result
}
