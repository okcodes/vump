package semver_test

import (
	"testing"

	bsemver "github.com/blang/semver/v4"
	"github.com/x/vump/internal/semver"
)

// helper to parse without boilerplate
func must(v bsemver.Version, err error) bsemver.Version {
	if err != nil {
		panic(err)
	}
	return v
}
func parse(s string) bsemver.Version {
	return must(bsemver.ParseTolerant(s))
}

// ─── BumpStable ────────────────────────────────────────────────────────────────

func TestBumpStable_Patch(t *testing.T) {
	got, err := semver.BumpStable(parse("1.2.3"), semver.BumpPatch)
	if err != nil || got.String() != "1.2.4" {
		t.Errorf("want 1.2.4, got %s, err %v", got, err)
	}
}

func TestBumpStable_Minor(t *testing.T) {
	got, err := semver.BumpStable(parse("1.2.3"), semver.BumpMinor)
	if err != nil || got.String() != "1.3.0" {
		t.Errorf("want 1.3.0, got %s, err %v", got, err)
	}
}

func TestBumpStable_Major(t *testing.T) {
	got, err := semver.BumpStable(parse("1.2.3"), semver.BumpMajor)
	if err != nil || got.String() != "2.0.0" {
		t.Errorf("want 2.0.0, got %s, err %v", got, err)
	}
}

func TestBumpStable_RejectsPreRelease(t *testing.T) {
	_, err := semver.BumpStable(parse("1.2.3-alpha.0"), semver.BumpPatch)
	if err == nil {
		t.Error("expected error when calling BumpStable on pre-release")
	}
}

// ─── StartPreRelease ───────────────────────────────────────────────────────────

func TestStartPreRelease_PatchAlpha(t *testing.T) {
	got, err := semver.StartPreRelease(parse("1.2.3"), semver.BumpPatch, "alpha")
	if err != nil || got.String() != "1.2.4-alpha.0" {
		t.Errorf("want 1.2.4-alpha.0, got %s, err %v", got, err)
	}
}

func TestStartPreRelease_MinorBeta(t *testing.T) {
	got, err := semver.StartPreRelease(parse("1.2.3"), semver.BumpMinor, "beta")
	if err != nil || got.String() != "1.3.0-beta.0" {
		t.Errorf("want 1.3.0-beta.0, got %s, err %v", got, err)
	}
}

func TestStartPreRelease_MajorRC(t *testing.T) {
	got, err := semver.StartPreRelease(parse("1.2.3"), semver.BumpMajor, "rc")
	if err != nil || got.String() != "2.0.0-rc.0" {
		t.Errorf("want 2.0.0-rc.0, got %s, err %v", got, err)
	}
}

// ─── BumpPreRelease ────────────────────────────────────────────────────────────

func TestBumpPreRelease_SameLabel_Increment(t *testing.T) {
	got, err := semver.BumpPreRelease(parse("1.2.3-alpha.0"), "alpha", false)
	if err != nil || got.String() != "1.2.3-alpha.1" {
		t.Errorf("want 1.2.3-alpha.1, got %s, err %v", got, err)
	}
}

func TestBumpPreRelease_AlphaToBeta(t *testing.T) {
	got, err := semver.BumpPreRelease(parse("1.2.3-alpha.2"), "beta", false)
	if err != nil || got.String() != "1.2.3-beta.0" {
		t.Errorf("want 1.2.3-beta.0, got %s, err %v", got, err)
	}
}

func TestBumpPreRelease_BetaToRC(t *testing.T) {
	got, err := semver.BumpPreRelease(parse("1.2.3-beta.1"), "rc", false)
	if err != nil || got.String() != "1.2.3-rc.0" {
		t.Errorf("want 1.2.3-rc.0, got %s, err %v", got, err)
	}
}

func TestBumpPreRelease_RCIncrement(t *testing.T) {
	got, err := semver.BumpPreRelease(parse("1.2.3-rc.1"), "rc", false)
	if err != nil || got.String() != "1.2.3-rc.2" {
		t.Errorf("want 1.2.3-rc.2, got %s, err %v", got, err)
	}
}

func TestBumpPreRelease_BackwardsError(t *testing.T) {
	_, err := semver.BumpPreRelease(parse("1.2.3-beta.0"), "alpha", false)
	if err == nil {
		t.Error("expected backwards error")
	}
	if !semver.IsBackwardsError(err) {
		t.Errorf("expected BackwardsError, got: %v", err)
	}
}

func TestBumpPreRelease_BackwardsWithForce(t *testing.T) {
	got, err := semver.BumpPreRelease(parse("1.2.3-beta.0"), "alpha", true)
	if err != nil {
		t.Errorf("expected no error with --force, got %v", err)
	}
	if got.String() != "1.2.3-alpha.0" {
		t.Errorf("want 1.2.3-alpha.0, got %s", got)
	}
}

func TestBumpPreRelease_RCToAlphaError(t *testing.T) {
	_, err := semver.BumpPreRelease(parse("1.2.3-rc.0"), "beta", false)
	if err == nil {
		t.Error("expected backwards error")
	}
}

// ─── DropPreRelease ────────────────────────────────────────────────────────────

func TestDropPreRelease(t *testing.T) {
	got := semver.DropPreRelease(parse("1.2.3-rc.1"))
	if got.String() != "1.2.3" {
		t.Errorf("want 1.2.3, got %s", got)
	}
}

// ─── IsStable ─────────────────────────────────────────────────────────────────

func TestIsStable(t *testing.T) {
	if !semver.IsStable(parse("1.2.3")) {
		t.Error("1.2.3 should be stable")
	}
	if semver.IsStable(parse("1.2.3-alpha.0")) {
		t.Error("1.2.3-alpha.0 should not be stable")
	}
}

// ─── ValidFromBase ─────────────────────────────────────────────────────────────

func TestValidFromBase(t *testing.T) {
	for _, valid := range []string{"patch", "minor", "major"} {
		if _, err := semver.ValidFromBase(valid); err != nil {
			t.Errorf("expected %q to be valid, got error: %v", valid, err)
		}
	}
	if _, err := semver.ValidFromBase("gamma"); err == nil {
		t.Error("expected error for invalid from-base 'gamma'")
	}
}

// ─── LabelFromBumpType ─────────────────────────────────────────────────────────

func TestLabelFromBumpType(t *testing.T) {
	cases := []struct {
		bt    semver.BumpType
		label string
		ok    bool
	}{
		{semver.BumpAlpha, "alpha", true},
		{semver.BumpBeta, "beta", true},
		{semver.BumpRC, "rc", true},
		{semver.BumpPatch, "", false},
		{semver.BumpRelease, "", false},
	}
	for _, c := range cases {
		l, ok := semver.LabelFromBumpType(c.bt)
		if ok != c.ok || l != c.label {
			t.Errorf("LabelFromBumpType(%q): want (%q, %v), got (%q, %v)", c.bt, c.label, c.ok, l, ok)
		}
	}
}

// ─── FormatStableReleaseOptions ────────────────────────────────────────────────

func TestFormatStableReleaseOptions(t *testing.T) {
	release, bumped, err := semver.FormatStableReleaseOptions(parse("1.2.3-rc.1"), semver.BumpPatch)
	if err != nil {
		t.Fatal(err)
	}
	if release.String() != "1.2.3" {
		t.Errorf("want release 1.2.3, got %s", release)
	}
	if bumped.String() != "1.2.4" {
		t.Errorf("want bumped 1.2.4, got %s", bumped)
	}
}
