package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	bsemver "github.com/blang/semver/v4"
	"github.com/spf13/cobra"

	"github.com/x/vump/internal/config"
	"github.com/x/vump/internal/git"
	"github.com/x/vump/internal/lockfile"
	"github.com/x/vump/internal/semver"
	"github.com/x/vump/internal/ui"
	"github.com/x/vump/internal/version"
)

var (
	flagDryRun bool
	flagForce  bool
	flagFrom   string
	flagCommit bool
	flagTag    bool
)

// Execute is the entry point called from main.
func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Fprintf(os.Stderr, "Error: %s\n", err)
		os.Exit(1)
	}
}

var rootCmd = &cobra.Command{
	Use:   "vump [patch|minor|major|alpha|beta|rc|release]",
	Short: "Bump versions across multiple files in a monorepo",
	Long: `vump bumps semver versions across package.json, Cargo.toml, and VERSION files
declared in vump.toml. Run without arguments for a fully interactive session.`,
	Args:          cobra.MaximumNArgs(1),
	RunE:          run,
	SilenceUsage:  true,
	SilenceErrors: true,
}

func init() {
	rootCmd.Flags().BoolVar(&flagDryRun, "dry-run", false, "Show what would change without writing anything")
	rootCmd.Flags().BoolVar(&flagForce, "force", false, "Bypass backwards pre-release guard")
	rootCmd.Flags().StringVar(&flagFrom, "from", "", "Base bump for pre-release from stable: patch, minor, or major")
	rootCmd.Flags().BoolVar(&flagCommit, "commit", false, "git add + commit after bumping")
	rootCmd.Flags().BoolVar(&flagTag, "tag", false, "git add + commit + tag after bumping (implies --commit)")
}

// fileVersionPair holds a file entry and its parsed version.
type fileVersionPair struct {
	entry   config.FileEntry
	absPath string
	ver     bsemver.Version
	rawVer  string
}

func run(cmd *cobra.Command, args []string) error {
	// Determine working directory.
	cwd, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("cannot determine working directory: %w", err)
	}

	// Load and validate config.
	cfg, err := config.Load(cwd)
	if err != nil {
		return err
	}

	// Resolve effective git flags (CLI overrides, --tag implies --commit).
	doCommit := flagCommit || cfg.Git.Commit
	doTag := flagTag || cfg.Git.Tag
	if doTag {
		doCommit = true
	}

	// Dirty tree check — before any prompts or writes.
	if doCommit && !flagDryRun {
		dirty, modified, err := git.IsDirty()
		if err != nil {
			return fmt.Errorf("checking git status: %w", err)
		}
		if dirty {
			fmt.Fprintln(os.Stderr, "✗ Uncommitted changes detected. Clean your working tree before using --commit or --tag.")
			fmt.Fprintln(os.Stderr, "\n  Modified:")
			for _, f := range modified {
				fmt.Fprintf(os.Stderr, "    %s\n", f)
			}
			fmt.Fprintln(os.Stderr, "\n  Run: git stash   or   git commit -m \"wip\"")
			os.Exit(1)
		}
	}

	// Read all versions from declared files.
	var pairs []fileVersionPair
	var parseErrors []string

	for _, f := range cfg.Files {
		absPath := filepath.Join(cwd, f.Path)
		raw, err := version.ReadVersion(absPath)
		if err != nil {
			parseErrors = append(parseErrors, fmt.Sprintf("  %s: %s", f.Path, err))
			continue
		}
		parsed, err := bsemver.ParseTolerant(raw)
		if err != nil {
			parseErrors = append(parseErrors, fmt.Sprintf("  %s: invalid semver %q: %s", f.Path, raw, err))
			continue
		}
		pairs = append(pairs, fileVersionPair{f, absPath, parsed, raw})
	}

	if len(parseErrors) > 0 {
		return fmt.Errorf("version parse errors:\n%s", strings.Join(parseErrors, "\n"))
	}

	// Determine base version (handle out-of-sync files).
	baseVer, err := resolveBase(pairs)
	if err != nil {
		return err
	}

	// Determine the bump type.
	var bumpType semver.BumpType
	if len(args) == 0 {
		// Fully interactive.
		chosen, err := ui.SelectBumpType(baseVer)
		if err != nil {
			return fmt.Errorf("prompt cancelled: %w", err)
		}
		bumpType = chosen
	} else {
		bumpType = semver.BumpType(args[0])
	}

	// Compute the new version.
	newVer, err := computeNewVersion(baseVer, bumpType)
	if err != nil {
		return err
	}

	// Confirm (only in fully interactive mode).
	if len(args) == 0 {
		confirmed, err := ui.Confirm(baseVer, newVer)
		if err != nil {
			return err
		}
		if !confirmed {
			fmt.Println("Aborted.")
			return nil
		}
	}

	newVerStr := newVer.String()

	// Dry-run: print and exit.
	if flagDryRun {
		printDryRun(pairs, newVerStr, doCommit, doTag, cfg)
		return nil
	}

	// Write all files.
	writtenPaths := make([]string, 0, len(pairs))
	for _, p := range pairs {
		if err := version.WriteVersion(p.absPath, newVerStr); err != nil {
			return fmt.Errorf("writing %s: %w", p.entry.Path, err)
		}
		writtenPaths = append(writtenPaths, p.absPath)
		fmt.Printf("✓ %s  %s  →  %s\n", p.entry.Path, p.rawVer, newVerStr)
	}

	// Lock file warnings.
	lockfile.CheckAndWarn(writtenPaths)

	// Git operations.
	if doCommit {
		relPaths := make([]string, len(pairs))
		for i, p := range pairs {
			relPaths[i] = p.entry.Path
		}
		if err := git.AddFiles(relPaths); err != nil {
			return err
		}
		commitMsg := config.FormatMessage(cfg.Git.CommitMessage, newVerStr)
		if err := git.Commit(commitMsg); err != nil {
			return err
		}
		fmt.Printf("\n✓ Committed: %s\n", commitMsg)

		if doTag {
			tagName := config.FormatMessage(cfg.Git.TagPattern, newVerStr)
			if err := git.Tag(tagName); err != nil {
				return err
			}
			fmt.Printf("✓ Tagged: %s\n", tagName)
			fmt.Printf("\n  To push:\n    git push && git push origin %s\n", tagName)
		} else {
			fmt.Println("\n  To push:\n    git push")
		}
	}

	return nil
}

// resolveBase returns the single base version, prompting if files are out of sync.
func resolveBase(pairs []fileVersionPair) (bsemver.Version, error) {
	if len(pairs) == 0 {
		return bsemver.Version{}, fmt.Errorf("no files to read version from")
	}
	base := pairs[0].ver
	allSame := true
	for _, p := range pairs[1:] {
		if p.ver.String() != base.String() {
			allSame = false
			break
		}
	}
	if allSame {
		return base, nil
	}
	// Out of sync — ask user interactively.
	vfiles := make([]ui.VersionedFile, len(pairs))
	for i, p := range pairs {
		vfiles[i] = ui.VersionedFile{Path: p.entry.Path, Version: p.ver}
	}
	return ui.SelectBaseVersion(vfiles)
}

// computeNewVersion applies bump rules from the spec.
func computeNewVersion(current bsemver.Version, bt semver.BumpType) (bsemver.Version, error) {
	isStable := semver.IsStable(current)
	label, isPre := semver.LabelFromBumpType(bt)

	switch {
	// release
	case bt == semver.BumpRelease:
		if isStable {
			return bsemver.Version{}, fmt.Errorf("already a stable version, nothing to release")
		}
		return semver.DropPreRelease(current), nil

	// patch/minor/major on stable
	case isStable && (bt == semver.BumpPatch || bt == semver.BumpMinor || bt == semver.BumpMajor):
		return semver.BumpStable(current, bt)

	// alpha/beta/rc on stable
	case isStable && isPre:
		if flagFrom != "" {
			baseBump, err := semver.ValidFromBase(flagFrom)
			if err != nil {
				return bsemver.Version{}, err
			}
			return semver.StartPreRelease(current, baseBump, label)
		}
		return ui.SelectPreReleaseBase(current, label)

	// patch/minor/major on pre-release
	case !isStable && (bt == semver.BumpPatch || bt == semver.BumpMinor || bt == semver.BumpMajor):
		return ui.SelectReleaseBehavior(current, bt)

	// alpha/beta/rc on pre-release
	case !isStable && isPre:
		return semver.BumpPreRelease(current, label, flagForce)
	}

	return bsemver.Version{}, fmt.Errorf("unhandled bump: %s on %s", bt, current)
}

// printDryRun outputs what would change, then exits.
func printDryRun(pairs []fileVersionPair, newVerStr string, doCommit, doTag bool, cfg *config.Config) {
	for _, p := range pairs {
		base := filepath.Base(p.entry.Path)
		switch base {
		case "package.json":
			fmt.Printf("[dry-run] Would write %s (.version):\n  %s  →  %s\n\n", p.entry.Path, p.rawVer, newVerStr)
		case "Cargo.toml":
			fmt.Printf("[dry-run] Would write %s ([package].version):\n  %s  →  %s\n\n", p.entry.Path, p.rawVer, newVerStr)
		default:
			fmt.Printf("[dry-run] Would write %s:\n  %s  →  %s\n\n", p.entry.Path, p.rawVer, newVerStr)
		}
	}
	if doCommit {
		relPaths := make([]string, len(pairs))
		for i, p := range pairs {
			relPaths[i] = p.entry.Path
		}
		commitMsg := config.FormatMessage(cfg.Git.CommitMessage, newVerStr)
		fmt.Printf("[dry-run] Would run: git add %s\n", strings.Join(relPaths, " "))
		fmt.Printf("[dry-run] Would run: git commit -m %q\n", commitMsg)
		if doTag {
			tagName := config.FormatMessage(cfg.Git.TagPattern, newVerStr)
			fmt.Printf("[dry-run] Would run: git tag %s\n", tagName)
		}
	}
	fmt.Println("\nNo files were modified.")
}
