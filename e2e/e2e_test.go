// Package e2e contains integration tests that run the vump binary end-to-end
// against real fixture files, exercising the CLI non-interactively.
package e2e_test

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

// vumpBin builds the binary once for the test session.
var vumpBin string

func TestMain(m *testing.M) {
	// Build the binary into a temp dir.
	bin, err := buildBinary()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to build vump: %v\n", err)
		os.Exit(1)
	}
	vumpBin = bin
	code := m.Run()
	os.Remove(bin)
	os.Exit(code)
}

func buildBinary() (string, error) {
	tmp, err := os.MkdirTemp("", "vump-e2e-bin-*")
	if err != nil {
		return "", err
	}
	binName := "vump"
	if runtime.GOOS == "windows" {
		binName = "vump.exe"
	}
	binPath := filepath.Join(tmp, binName)

	// Find module root (two levels up from this file: e2e/ → vump/).
	_, file, _, _ := runtime.Caller(0)
	moduleRoot := filepath.Join(filepath.Dir(file), "..")

	cmd := exec.Command("go", "build", "-o", binPath, ".")
	cmd.Dir = moduleRoot
	out, err := cmd.CombinedOutput()
	if err != nil {
		return "", fmt.Errorf("build failed: %s", out)
	}
	return binPath, nil
}

// ─── Fixture helpers ──────────────────────────────────────────────────────────

type fixture struct {
	dir string
	t   *testing.T
}

func newFixture(t *testing.T) *fixture {
	t.Helper()
	dir := t.TempDir()
	return &fixture{dir: dir, t: t}
}

func (f *fixture) write(name, content string) *fixture {
	f.t.Helper()
	if err := os.WriteFile(filepath.Join(f.dir, name), []byte(content), 0644); err != nil {
		f.t.Fatal(err)
	}
	return f
}

func (f *fixture) mkdir(name string) *fixture {
	f.t.Helper()
	if err := os.MkdirAll(filepath.Join(f.dir, name), 0755); err != nil {
		f.t.Fatal(err)
	}
	return f
}

func (f *fixture) vump(args ...string) (string, string, error) {
	cmd := exec.Command(vumpBin, args...)
	cmd.Dir = f.dir
	var stdout, stderr strings.Builder
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	err := cmd.Run()
	return stdout.String(), stderr.String(), err
}

func (f *fixture) readFile(name string) string {
	f.t.Helper()
	data, err := os.ReadFile(filepath.Join(f.dir, name))
	if err != nil {
		f.t.Fatalf("readFile %s: %v", name, err)
	}
	return strings.TrimSpace(string(data))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

func TestE2E_PatchBump_VERSION(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.2.3\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("patch")
	if err != nil {
		t.Fatalf("vump patch failed: %v\nstdout: %s", err, stdout)
	}
	got := fx.readFile("VERSION")
	if got != "1.2.4" {
		t.Errorf("want VERSION=1.2.4, got %q", got)
	}
}

func TestE2E_MinorBump_PackageJSON(t *testing.T) {
	fx := newFixture(t).
		write("package.json", `{"name":"app","version":"0.3.1"}`+"\n").
		write("vump.toml", `[[files]]
path = "package.json"
`)
	stdout, _, err := fx.vump("minor")
	if err != nil {
		t.Fatalf("vump minor failed: %v\nstdout: %s", err, stdout)
	}
	got := fx.readFile("package.json")
	if !strings.Contains(got, `"version": "0.4.0"`) {
		t.Errorf("want version 0.4.0 in package.json, got:\n%s", got)
	}
}

func TestE2E_MajorBump_CargoToml(t *testing.T) {
	fx := newFixture(t).
		write("Cargo.toml", "[package]\nname = \"crate\"\nversion = \"0.1.0\"\n").
		write("vump.toml", `[[files]]
path = "Cargo.toml"
`)
	stdout, _, err := fx.vump("major")
	if err != nil {
		t.Fatalf("vump major failed: %v\nstdout: %s", err, stdout)
	}
	got := fx.readFile("Cargo.toml")
	if !strings.Contains(got, `version = "1.0.0"`) {
		t.Errorf("want version 1.0.0 in Cargo.toml, got:\n%s", got)
	}
}

func TestE2E_AlphaFromStable_WithFrom(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("alpha", "--from", "minor")
	if err != nil {
		t.Fatalf("vump alpha --from minor failed: %v\nstdout: %s", err, stdout)
	}
	got := fx.readFile("VERSION")
	if got != "1.1.0-alpha.0" {
		t.Errorf("want 1.1.0-alpha.0, got %q", got)
	}
}

func TestE2E_AlphaIncrement(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0-alpha.2\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("alpha")
	if err != nil {
		t.Fatalf("vump alpha failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "1.0.0-alpha.3" {
		t.Errorf("got %q", fx.readFile("VERSION"))
	}
}

func TestE2E_BetaFromAlpha(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0-alpha.5\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("beta")
	if err != nil {
		t.Fatalf("vump beta failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "1.0.0-beta.0" {
		t.Errorf("got %q", fx.readFile("VERSION"))
	}
}

func TestE2E_Release_DropsPreRelease(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "2.0.0-rc.3\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("release")
	if err != nil {
		t.Fatalf("vump release failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "2.0.0" {
		t.Errorf("got %q", fx.readFile("VERSION"))
	}
}

func TestE2E_Release_ErrorOnStable(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, stderr, err := fx.vump("release")
	if err == nil {
		t.Errorf("expected error when releasing stable version\nstdout: %s", stdout)
	}
	if !strings.Contains(stderr+stdout, "stable") {
		t.Errorf("expected 'stable' in output, got stdout=%q stderr=%q", stdout, stderr)
	}
}

func TestE2E_BackwardsGuard_Error(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0-beta.0\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	_, stderr, err := fx.vump("alpha")
	if err == nil {
		t.Error("expected error when going backwards (beta→alpha)")
	}
	if !strings.Contains(stderr+"\n", "force") && !strings.Contains(stderr, "backwards") {
		t.Errorf("expected force/backwards mention in stderr: %q", stderr)
	}
}

func TestE2E_BackwardsGuard_Force(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "1.0.0-beta.0\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("alpha", "--force")
	if err != nil {
		t.Fatalf("vump alpha --force failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "1.0.0-alpha.0" {
		t.Errorf("got %q", fx.readFile("VERSION"))
	}
}

func TestE2E_DryRun_WritesNothing(t *testing.T) {
	original := "0.5.0\n"
	fx := newFixture(t).
		write("VERSION", original).
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("patch", "--dry-run")
	if err != nil {
		t.Fatalf("dry-run failed: %v\nstdout: %s", err, stdout)
	}
	// File must be unchanged.
	if got := fx.readFile("VERSION"); got != strings.TrimSpace(original) {
		t.Errorf("dry-run modified VERSION: got %q", got)
	}
	if !strings.Contains(stdout, "[dry-run]") {
		t.Errorf("expected [dry-run] in output: %s", stdout)
	}
	if !strings.Contains(stdout, "No files were modified") {
		t.Errorf("expected 'No files were modified' in output: %s", stdout)
	}
}

func TestE2E_MultipleFiles_AllUpdated(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "0.1.0\n").
		write("package.json", `{"name":"app","version":"0.1.0"}`+"\n").
		write("vump.toml", `
[[files]]
path = "VERSION"

[[files]]
path = "package.json"
`)
	stdout, _, err := fx.vump("patch")
	if err != nil {
		t.Fatalf("patch failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "0.1.1" {
		t.Errorf("VERSION: got %q", fx.readFile("VERSION"))
	}
	pkgContent := fx.readFile("package.json")
	if !strings.Contains(pkgContent, `"version": "0.1.1"`) {
		t.Errorf("package.json: got %s", pkgContent)
	}
}

func TestE2E_MissingFile_Error(t *testing.T) {
	fx := newFixture(t).
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	// VERSION not created
	_, stderr, err := fx.vump("patch")
	if err == nil {
		t.Error("expected error for missing VERSION")
	}
	if !strings.Contains(stderr+"\n", "File not found") && !strings.Contains(stderr, "VERSION") {
		t.Errorf("expected clear error message, got: %q", stderr)
	}
}

func TestE2E_UnsupportedFile_Error(t *testing.T) {
	fx := newFixture(t).
		write("go.mod", "module foo\n").
		write("vump.toml", `[[files]]
path = "go.mod"
`)
	_, stderr, err := fx.vump("patch")
	if err == nil {
		t.Error("expected error for unsupported file type")
	}
	if !strings.Contains(stderr, "go.mod") {
		t.Errorf("expected go.mod in error output: %q", stderr)
	}
}

func TestE2E_InvalidSemver_Error(t *testing.T) {
	fx := newFixture(t).
		write("VERSION", "not-a-version\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	_, _, err := fx.vump("patch")
	if err == nil {
		t.Error("expected error for invalid semver in VERSION")
	}
}

func TestE2E_RC_FromAlpha_SkipsBeta(t *testing.T) {
	// Going from alpha directly to rc should work (rc > alpha).
	fx := newFixture(t).
		write("VERSION", "1.0.0-alpha.3\n").
		write("vump.toml", `[[files]]
path = "VERSION"
`)
	stdout, _, err := fx.vump("rc")
	if err != nil {
		t.Fatalf("vump rc failed: %v\nstdout: %s", err, stdout)
	}
	if fx.readFile("VERSION") != "1.0.0-rc.0" {
		t.Errorf("got %q", fx.readFile("VERSION"))
	}
}
