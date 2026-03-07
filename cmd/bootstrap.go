package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/charmbracelet/huh"
	"github.com/okcodes/vump/internal/config"
	"github.com/okcodes/vump/internal/ui"
	"github.com/okcodes/vump/internal/version"
	"github.com/pelletier/go-toml/v2"
	"github.com/spf13/cobra"
)

var flagBootstrapForce bool
var flagBootstrapNoScan bool

var bootstrapCmd = &cobra.Command{
	Use:   "bootstrap",
	Short: "Create an initial vump.toml interactively",
	Long: `bootstrap scans the current directory for version files, presents them for
selection, asks about git integration, and writes a ready-to-use vump.toml.

Run this once at the root of a new project. Use --force to overwrite an
existing vump.toml.`,
	Args:          cobra.NoArgs,
	RunE:          runBootstrap,
	SilenceUsage:  true,
	SilenceErrors: true,
}

func init() {
	rootCmd.AddCommand(bootstrapCmd)
	bootstrapCmd.Flags().BoolVar(&flagBootstrapForce, "force", false, "Overwrite an existing vump.toml")
	bootstrapCmd.Flags().BoolVar(&flagBootstrapNoScan, "no-scan", false, "Skip file discovery; start with an empty list")
}

// ignoredDirs are never entered during scanning.
var ignoredDirs = map[string]bool{
	"node_modules": true,
	".git":         true,
	"dist":         true,
	"build":        true,
	"target":       true,
	".cache":       true,
	"vendor":       true,
	"out":          true,
	".next":        true,
	".nuxt":        true,
	"coverage":     true,
}

type candidate struct {
	relPath string
	ver     string // empty if unreadable
}

func runBootstrap(_ *cobra.Command, _ []string) error {
	cwd, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("cannot determine working directory: %w", err)
	}

	const outputFile = "vump.toml"
	outputPath := filepath.Join(cwd, outputFile)

	if _, err := os.Stat(outputPath); err == nil && !flagBootstrapForce {
		return fmt.Errorf("%s already exists — run with --force to overwrite", outputFile)
	}

	// ── Discover candidate files ──────────────────────────────────────────────
	var found []candidate
	if !flagBootstrapNoScan {
		fmt.Println("Scanning for version files…")
		found = scanForVersionFiles(cwd, 3)
	}

	// ── Select files ──────────────────────────────────────────────────────────
	selectedPaths, err := selectFiles(found)
	if err != nil {
		return err
	}

	// If the user deselected everything from the multi-select, fall through
	// to manual path entry — same experience as when no files were discovered.
	if len(selectedPaths) == 0 {
		fmt.Println("  No files selected. Enter paths relative to the repo root.")
		fmt.Println("  (e.g. VERSION, packages/ui/package.json, api/Cargo.toml)")
		fmt.Println()
		selectedPaths, err = enterPathsManually()
		if err != nil {
			return err
		}
	}

	if len(selectedPaths) == 0 {
		return fmt.Errorf("at least one file is required — add a [[files]] entry to vump.toml manually")
	}

	// ── Version mismatch warning ──────────────────────────────────────────────
	warnIfVersionMismatch(cwd, selectedPaths)

	// ── Git action ────────────────────────────────────────────────────────────
	gitAction, err := ui.SelectGitAction(ui.GitActionNone)
	if err != nil {
		return fmt.Errorf("prompt cancelled: %w", err)
	}

	doPush := false
	if gitAction != ui.GitActionNone {
		doPush, err = ui.ConfirmPush(false)
		if err != nil {
			return fmt.Errorf("prompt cancelled: %w", err)
		}
	}

	// ── Build config struct and marshal to TOML ───────────────────────────────
	cfg := buildConfig(selectedPaths, gitAction, doPush)
	if err := writeConfig(outputPath, cfg); err != nil {
		return fmt.Errorf("writing %s: %w", outputFile, err)
	}

	fmt.Printf("\n✓ Created %s\n", outputFile)
	fmt.Println("  Run \"vump\" to bump versions interactively.")
	return nil
}

// scanForVersionFiles walks root up to maxDepth levels deep, skipping
// ignoredDirs, and returns files named package.json / Cargo.toml / VERSION
// that contain a readable version field.
func scanForVersionFiles(root string, maxDepth int) []candidate {
	var results []candidate
	var walk func(dir string, depth int)
	walk = func(dir string, depth int) {
		if depth > maxDepth {
			return
		}
		entries, err := os.ReadDir(dir)
		if err != nil {
			return
		}
		for _, e := range entries {
			name := e.Name()
			if e.IsDir() {
				if ignoredDirs[name] || strings.HasPrefix(name, ".") {
					continue
				}
				walk(filepath.Join(dir, name), depth+1)
				continue
			}
			if name != "package.json" && name != "Cargo.toml" && name != "VERSION" {
				continue
			}
			absPath := filepath.Join(dir, name)
			rel, _ := filepath.Rel(root, absPath)
			rel = filepath.ToSlash(rel)
			ver, _ := version.ReadVersion(absPath)
			results = append(results, candidate{relPath: rel, ver: ver})
		}
	}
	walk(root, 0)
	return results
}

// selectFiles shows a multi-select when files were discovered,
// or delegates to enterPathsManually when nothing was found.
func selectFiles(found []candidate) ([]string, error) {
	if len(found) > 0 {
		options := make([]huh.Option[string], len(found))
		for i, c := range found {
			label := c.relPath
			if c.ver != "" {
				label = fmt.Sprintf("%-40s (v%s)", c.relPath, c.ver)
			}
			options[i] = huh.NewOption(label, c.relPath).Selected(true)
		}

		var confirmed []string
		form := huh.NewForm(
			huh.NewGroup(
				huh.NewMultiSelect[string]().
					Title("Select files for vump to track:").
					Options(options...).
					Value(&confirmed),
			),
		)
		if err := form.Run(); err != nil {
			return nil, fmt.Errorf("prompt cancelled: %w", err)
		}
		return confirmed, nil
	}

	// No files found — manual entry.
	fmt.Println("  No version files found. Enter paths relative to the repo root.")
	fmt.Println("  (e.g. VERSION, packages/ui/package.json, api/Cargo.toml)")
	fmt.Println()
	return enterPathsManually()
}

// enterPathsManually shows a single prompt for comma-separated file paths.
func enterPathsManually() ([]string, error) {
	var input string
	form := huh.NewForm(
		huh.NewGroup(
			huh.NewInput().
				Title("Enter file paths (comma-separated):").
				Placeholder("VERSION, packages/ui/package.json, api/Cargo.toml").
				Value(&input),
		),
	)
	if err := form.Run(); err != nil {
		return nil, fmt.Errorf("prompt cancelled: %w", err)
	}
	var paths []string
	for _, part := range strings.Split(input, ",") {
		p := strings.TrimSpace(filepath.ToSlash(part))
		if p != "" {
			paths = append(paths, p)
		}
	}
	return paths, nil
}

// warnIfVersionMismatch reads the version from each selected file and prints
// a warning if they differ.
func warnIfVersionMismatch(root string, paths []string) {
	if len(paths) < 2 {
		return
	}
	byVer := make(map[string][]string)
	for _, rel := range paths {
		abs := filepath.Join(root, filepath.FromSlash(rel))
		ver, err := version.ReadVersion(abs)
		if err != nil || ver == "" {
			continue
		}
		byVer[ver] = append(byVer[ver], rel)
	}
	if len(byVer) <= 1 {
		return
	}
	fmt.Fprintln(os.Stderr, "\n⚠  Version mismatch across selected files:")
	for ver, ps := range byVer {
		for _, p := range ps {
			fmt.Fprintf(os.Stderr, "   %-40s  %s\n", p, ver)
		}
	}
	fmt.Fprintln(os.Stderr, "   Run \"vump\" after bootstrap to resolve the mismatch.")
}

// buildConfig constructs a config.Config from the user's bootstrap choices.
func buildConfig(paths []string, gitAction ui.GitAction, doPush bool) config.Config {
	doCommit := gitAction == ui.GitActionCommit || gitAction == ui.GitActionTag
	doTag := gitAction == ui.GitActionTag

	files := make([]config.FileEntry, len(paths))
	for i, p := range paths {
		files[i] = config.FileEntry{Path: p}
	}

	return config.Config{
		Git: config.GitConfig{
			Commit:        doCommit,
			CommitMessage: config.DefaultCommitMessage,
			Tag:           doTag,
			TagPattern:    config.DefaultTagPattern,
			Push:          doPush,
		},
		Files: files,
	}
}

// writeConfig serializes cfg to a TOML file at outputPath.
// A short header comment is prepended; the struct is marshaled by the TOML library.
func writeConfig(outputPath string, cfg config.Config) error {
	body, err := toml.Marshal(cfg)
	if err != nil {
		return fmt.Errorf("marshaling config: %w", err)
	}

	header := `# vump.toml — generated by "vump bootstrap"
# Place this file at the root of your monorepo.
# CLI flags (--commit / --tag / --push) override the [git] settings below.

`
	return os.WriteFile(outputPath, append([]byte(header), body...), 0644)
}
