package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	bsemver "github.com/blang/semver/v4"
	"github.com/spf13/cobra"

	"github.com/x/vump/internal/config"
	"github.com/x/vump/internal/version"
)

var checkCmd = &cobra.Command{
	Use:   "check <version>",
	Short: "Verify that all tracked files match the given version",
	Long: `check reads every file declared in vump.toml and compares its version
against the expected version. Exits 0 silently if all files match,
exits 1 with a clear report if any file does not.

Leading "v" prefix is accepted and ignored (v0.1.0 == 0.1.0).

Intended for CI/CD pipelines, e.g. to verify a git tag matches source.`,
	Args:          cobra.ExactArgs(1),
	RunE:          runCheck,
	SilenceUsage:  true,
	SilenceErrors: true,
}

func init() {
	rootCmd.AddCommand(checkCmd)
}

func runCheck(_ *cobra.Command, args []string) error {
	cwd, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("cannot determine working directory: %w", err)
	}

	// Strip optional leading "v".
	raw := strings.TrimPrefix(args[0], "v")

	// Validate that the argument is a proper semver string.
	expected, err := bsemver.ParseTolerant(raw)
	if err != nil {
		return fmt.Errorf("invalid version %q: %s", args[0], err)
	}
	expectedStr := expected.String()

	// Load config (runs all startup validations).
	cfg, err := config.Load(cwd)
	if err != nil {
		return err
	}

	// Read version from every declared file.
	type mismatch struct {
		path  string
		found string
	}
	var mismatches []mismatch

	for _, f := range cfg.Files {
		absPath := filepath.Join(cwd, f.Path)
		found, err := version.ReadVersion(absPath)
		if err != nil {
			return fmt.Errorf("reading %s: %w", f.Path, err)
		}
		// Normalise through semver so "0.1.0" and "v0.1.0" compare equal.
		parsedFound, err := bsemver.ParseTolerant(found)
		if err != nil {
			return fmt.Errorf("%s: invalid semver %q: %s", f.Path, found, err)
		}
		if parsedFound.String() != expectedStr {
			mismatches = append(mismatches, mismatch{f.Path, parsedFound.String()})
		}
	}

	// All good — silent success.
	if len(mismatches) == 0 {
		return nil
	}

	// Print a clear mismatch report to stderr and exit 1.
	fmt.Fprintf(os.Stderr, "✗  Version mismatch (expected: %s)\n\n", expectedStr)

	// Column-align file paths for readability.
	maxLen := 0
	for _, m := range mismatches {
		if len(m.path) > maxLen {
			maxLen = len(m.path)
		}
	}
	for _, m := range mismatches {
		padding := strings.Repeat(" ", maxLen-len(m.path))
		fmt.Fprintf(os.Stderr, "   %s%s   found %s\n", m.path, padding, m.found)
	}
	fmt.Fprintln(os.Stderr)

	// Use os.Exit so the caller gets exit code 1 cleanly.
	os.Exit(1)
	return nil
}
