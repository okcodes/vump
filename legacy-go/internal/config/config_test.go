package config_test

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/okcodes/vump/internal/config"
)

func writeFile(t *testing.T, dir, name, content string) {
	t.Helper()
	if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0644); err != nil {
		t.Fatal(err)
	}
}

func TestLoad_ValidMinimal(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "VERSION", "1.0.0\n")
	writeFile(t, dir, "vump.toml", `
[[files]]
path = "VERSION"
`)
	cfg, err := config.Load(dir)
	if err != nil {
		t.Fatalf("expected no error, got %v", err)
	}
	if len(cfg.Files) != 1 {
		t.Errorf("want 1 file, got %d", len(cfg.Files))
	}
}

func TestLoad_WithGitSection(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "VERSION", "1.0.0\n")
	writeFile(t, dir, "vump.toml", `
[git]
commit = true
commit_message = "chore: release v{new_version}"
tag = true
tag_pattern = "v{new_version}"

[[files]]
path = "VERSION"
`)
	cfg, err := config.Load(dir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !cfg.Git.Commit {
		t.Error("expected git.commit = true")
	}
	if !cfg.Git.Tag {
		t.Error("expected git.tag = true")
	}
	if cfg.Git.CommitMessage != "chore: release v{new_version}" {
		t.Errorf("unexpected commit message: %q", cfg.Git.CommitMessage)
	}
}

func TestLoad_DefaultCommitMessage(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "VERSION", "1.0.0\n")
	writeFile(t, dir, "vump.toml", `[[files]]
path = "VERSION"
`)
	cfg, err := config.Load(dir)
	if err != nil {
		t.Fatal(err)
	}
	if cfg.Git.CommitMessage != config.DefaultCommitMessage {
		t.Errorf("want default commit message, got %q", cfg.Git.CommitMessage)
	}
}

func TestLoad_MissingToml(t *testing.T) {
	dir := t.TempDir()
	_, err := config.Load(dir)
	if err == nil {
		t.Error("expected error for missing vump.toml")
	}
}

func TestLoad_NoFiles(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "vump.toml", ``)
	_, err := config.Load(dir)
	if err == nil {
		t.Error("expected error when no files declared")
	}
}

func TestLoad_MissingFile(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "vump.toml", `[[files]]
path = "VERSION"
`)
	// VERSION not created
	_, err := config.Load(dir)
	if err == nil {
		t.Error("expected error for missing file on disk")
	}
}

func TestLoad_UnsupportedFileType(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "go.mod", "module foo\n")
	writeFile(t, dir, "vump.toml", `[[files]]
path = "go.mod"
`)
	_, err := config.Load(dir)
	if err == nil {
		t.Error("expected error for unsupported file type")
	}
}

func TestLoad_MultipleFiles(t *testing.T) {
	dir := t.TempDir()
	writeFile(t, dir, "VERSION", "0.1.0\n")

	pkgJSON := `{"name":"app","version":"0.1.0"}`
	writeFile(t, dir, "package.json", pkgJSON)

	writeFile(t, dir, "vump.toml", `
[[files]]
path = "VERSION"

[[files]]
path = "package.json"
`)
	cfg, err := config.Load(dir)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(cfg.Files) != 2 {
		t.Errorf("want 2 files, got %d", len(cfg.Files))
	}
}

// ─── FormatMessage ─────────────────────────────────────────────────────────────

func TestFormatMessage(t *testing.T) {
	got := config.FormatMessage("chore: bump to v{new_version}", "1.2.3")
	want := "chore: bump to v1.2.3"
	if got != want {
		t.Errorf("want %q, got %q", want, got)
	}
}
