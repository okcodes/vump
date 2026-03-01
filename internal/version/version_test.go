package version_test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/x/vump/internal/version"
)

// ─── DetectType ───────────────────────────────────────────────────────────────

func TestDetectType_Known(t *testing.T) {
	cases := []struct {
		name string
		want version.FileType
	}{
		{"package.json", version.FileTypePackageJSON},
		{"Cargo.toml", version.FileTypeCargoToml},
		{"VERSION", version.FileTypeVersionFile},
	}
	for _, c := range cases {
		ft, err := version.DetectType(c.name)
		if err != nil {
			t.Errorf("DetectType(%q): unexpected error: %v", c.name, err)
		}
		if ft != c.want {
			t.Errorf("DetectType(%q): want %v, got %v", c.name, c.want, ft)
		}
	}
}

func TestDetectType_Unknown(t *testing.T) {
	for _, name := range []string{"go.mod", "pyproject.toml", "version.txt", "CARGO.TOML"} {
		if _, err := version.DetectType(name); err == nil {
			t.Errorf("expected error for unsupported file %q", name)
		}
	}
}

// ─── package.json: key order preserved ───────────────────────────────────────

func TestPackageJSONRoundTrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "package.json")

	// Keys are intentionally non-alphabetical to prove order is preserved.
	initial := `{
  "name": "my-app",
  "version": "1.0.0",
  "scripts": { "dev": "vite" },
  "dependencies": { "react": "^19.0.0" },
  "devDependencies": {}
}
`
	if err := os.WriteFile(path, []byte(initial), 0644); err != nil {
		t.Fatal(err)
	}

	// Read.
	got, err := version.ReadVersion(path)
	if err != nil || got != "1.0.0" {
		t.Fatalf("ReadVersion: want 1.0.0, got %q, err %v", got, err)
	}

	// Write new version.
	if err := version.WriteVersion(path, "2.0.0-beta.1"); err != nil {
		t.Fatalf("WriteVersion: %v", err)
	}

	data, _ := os.ReadFile(path)
	content := string(data)

	// Version field must be updated.
	if !strings.Contains(content, `"version": "2.0.0-beta.1"`) {
		t.Errorf("version not updated:\n%s", content)
	}

	// Key order must be preserved: name → version → scripts → dependencies.
	nameIdx := strings.Index(content, `"name"`)
	versionIdx := strings.Index(content, `"version"`)
	scriptsIdx := strings.Index(content, `"scripts"`)
	depsIdx := strings.Index(content, `"dependencies"`)
	if !(nameIdx < versionIdx && versionIdx < scriptsIdx && scriptsIdx < depsIdx) {
		t.Errorf("key order not preserved:\n%s", content)
	}
}

// ─── Cargo.toml round-trip ────────────────────────────────────────────────────

func TestCargoTomlRoundTrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "Cargo.toml")

	initial := `[package]
name = "my-crate"
version = "0.1.0"
edition = "2021"
`
	if err := os.WriteFile(path, []byte(initial), 0644); err != nil {
		t.Fatal(err)
	}

	got, err := version.ReadVersion(path)
	if err != nil || got != "0.1.0" {
		t.Fatalf("ReadVersion: want 0.1.0, got %q, err %v", got, err)
	}

	if err := version.WriteVersion(path, "1.2.3-rc.0"); err != nil {
		t.Fatalf("WriteVersion: %v", err)
	}

	got2, err := version.ReadVersion(path)
	if err != nil || got2 != "1.2.3-rc.0" {
		t.Errorf("after write: want 1.2.3-rc.0, got %q, err %v", got2, err)
	}

	// Ensure other fields were preserved.
	data, _ := os.ReadFile(path)
	content := string(data)
	if !strings.Contains(content, `name = "my-crate"`) || !strings.Contains(content, `edition = "2021"`) {
		t.Errorf("Cargo.toml fields lost after write:\n%s", content)
	}
}

// ─── VERSION round-trip ───────────────────────────────────────────────────────

func TestVERSIONRoundTrip(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "VERSION")

	if err := os.WriteFile(path, []byte("0.5.0\n"), 0644); err != nil {
		t.Fatal(err)
	}

	got, err := version.ReadVersion(path)
	if err != nil || got != "0.5.0" {
		t.Fatalf("ReadVersion: want 0.5.0, got %q, err %v", got, err)
	}

	if err := version.WriteVersion(path, "0.6.0-alpha.0"); err != nil {
		t.Fatalf("WriteVersion: %v", err)
	}

	got2, err := version.ReadVersion(path)
	if err != nil || got2 != "0.6.0-alpha.0" {
		t.Errorf("after write: want 0.6.0-alpha.0, got %q, err %v", got2, err)
	}
}

// ─── Error cases ──────────────────────────────────────────────────────────────

func TestReadVersion_MissingVersionField(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "package.json")
	os.WriteFile(path, []byte(`{"name":"foo"}`), 0644)
	_, err := version.ReadVersion(path)
	if err == nil {
		t.Error("expected error when version field missing")
	}
}

func TestReadVersion_EmptyVERSION(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "VERSION")
	os.WriteFile(path, []byte("   \n"), 0644)
	_, err := version.ReadVersion(path)
	if err == nil {
		t.Error("expected error for empty VERSION file")
	}
}
