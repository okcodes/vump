package config

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/okcodes/vump/internal/version"
	"github.com/pelletier/go-toml/v2"
)

// Config is the top-level structure for vump.toml.
type Config struct {
	Git   GitConfig   `toml:"git"`
	Files []FileEntry `toml:"files"`
}

// GitConfig holds optional git integration settings.
type GitConfig struct {
	Commit        bool   `toml:"commit"`
	CommitMessage string `toml:"commit_message"`
	Tag           bool   `toml:"tag"`
	TagPattern    string `toml:"tag_pattern"`
	Push          bool   `toml:"push"`
}

// FileEntry represents a single version file tracked by vump.
type FileEntry struct {
	Path string `toml:"path"`
}

const DefaultCommitMessage = "chore: bump version to v{new_version}"
const DefaultTagPattern = "v{new_version}"

// Load reads and validates vump.toml from the given directory.
// All startup checks are run here, before any interactive logic.
func Load(dir string) (*Config, error) {
	cfgPath := filepath.Join(dir, "vump.toml")

	data, err := os.ReadFile(cfgPath)
	if err != nil {
		if errors.Is(err, os.ErrNotExist) {
			return nil, fmt.Errorf("vump.toml not found in %s — create one to get started", dir)
		}
		return nil, fmt.Errorf("reading vump.toml: %w", err)
	}

	var cfg Config
	if err := toml.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parsing vump.toml: %w", err)
	}

	// Apply defaults for git config.
	if cfg.Git.CommitMessage == "" {
		cfg.Git.CommitMessage = DefaultCommitMessage
	}
	if cfg.Git.TagPattern == "" {
		cfg.Git.TagPattern = DefaultTagPattern
	}

	// Validation: at least one file declared.
	if len(cfg.Files) == 0 {
		return nil, fmt.Errorf("vump.toml has no [[files]] entries — declare at least one file")
	}

	// Validation: all declared files must exist and be supported types.
	var missingFiles []string
	var unsupportedFiles []string

	for _, f := range cfg.Files {
		absPath := filepath.Join(dir, f.Path)

		// Check file existence.
		if _, err := os.Stat(absPath); errors.Is(err, os.ErrNotExist) {
			missingFiles = append(missingFiles, f.Path)
			continue
		}

		// Check supported filename.
		base := filepath.Base(f.Path)
		if _, err := version.DetectType(base); err != nil {
			unsupportedFiles = append(unsupportedFiles, f.Path)
		}
	}

	if len(missingFiles) > 0 {
		msgs := make([]string, len(missingFiles))
		for i, p := range missingFiles {
			msgs[i] = fmt.Sprintf("  File not found: %s — create it or remove it from vump.toml", p)
		}
		return nil, fmt.Errorf("missing files:\n%s", strings.Join(msgs, "\n"))
	}

	if len(unsupportedFiles) > 0 {
		msgs := make([]string, len(unsupportedFiles))
		for i, p := range unsupportedFiles {
			msgs[i] = fmt.Sprintf("  %s (supported: package.json, Cargo.toml, VERSION)", p)
		}
		return nil, fmt.Errorf("unsupported file types:\n%s", strings.Join(msgs, "\n"))
	}

	return &cfg, nil
}

// FormatMessage replaces {new_version} in a template string.
func FormatMessage(template, newVersion string) string {
	return strings.ReplaceAll(template, "{new_version}", newVersion)
}
