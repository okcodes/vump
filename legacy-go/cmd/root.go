package cmd

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	bsemver "github.com/blang/semver/v4"
	"github.com/spf13/cobra"

	"github.com/okcodes/vump/internal/config"
	"github.com/okcodes/vump/internal/git"
	"github.com/okcodes/vump/internal/lockfile"
	"github.com/okcodes/vump/internal/semver"
	"github.com/okcodes/vump/internal/ui"
	"github.com/okcodes/vump/internal/version"
)

var (
	flagDryRun bool
	flagForce  bool
	flagFrom   string
	flagCommit bool
	flagTag    bool
	flagPush   bool
)

// Execute is the entry point called from main.
// version is the value injected at build time via ldflags.
func Execute(version string) {
	rootCmd.Version = version
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
	rootCmd.Flags().BoolVar(&flagPush, "push", false, "Push commit (and tag) to remote after bumping (implies --commit)")
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

	// Track whether the user explicitly passed a git flag on the CLI.
	// Explicit flag = the user has already answered the question.
	explicitGitFlag := flagCommit || flagTag
	explicitPushFlag := flagPush

	// Starting values from CLI flags only.
	doCommit := flagCommit
	doTag := flagTag
	if flagPush {
		doCommit = true // --push implies at least a commit
	}
	if doTag {
		doCommit = true
	}
	doPush := flagPush

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
		// Fully interactive: ask the user.
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

	// ── Interactive path (no bump type arg given) ─────────────────────────────
	if len(args) == 0 {
		// Git action: only prompt if the user hasn't already said so via a flag.
		// If they did pass --commit/--tag, honour it and skip the question.
		var gitAction ui.GitAction
		if explicitGitFlag {
			// Map the CLI flags to a GitAction so Confirm can display it.
			if doTag {
				gitAction = ui.GitActionTag
			} else {
				gitAction = ui.GitActionCommit
			}
		} else {
			// No explicit flag: ask interactively, pre-selecting the config default.
			var configDefault ui.GitAction
			if cfg.Git.Tag {
				configDefault = ui.GitActionTag
			} else if cfg.Git.Commit {
				configDefault = ui.GitActionCommit
			}
			gitAction, err = ui.SelectGitAction(configDefault)
			if err != nil {
				return fmt.Errorf("prompt cancelled: %w", err)
			}
			// Apply the user's interactive choice.
			doCommit = gitAction == ui.GitActionCommit || gitAction == ui.GitActionTag
			doTag = gitAction == ui.GitActionTag
		}

		// Push prompt — only if commit or tag will happen.
		if doCommit {
			if explicitPushFlag {
				doPush = true
			} else {
				doPush, err = ui.ConfirmPush(cfg.Git.Push)
				if err != nil {
					return fmt.Errorf("prompt cancelled: %w", err)
				}
			}
		}

		// Dirty tree check — now that we know whether commit/tag will happen.
		if doCommit && !flagDryRun {
			dirty, modified, gitErr := git.IsDirty()
			if gitErr != nil {
				return fmt.Errorf("checking git status: %w", gitErr)
			}
			if dirty {
				fmt.Fprintln(os.Stderr, "\n✗ Uncommitted changes detected. Cannot commit/tag with unclean working tree.")
				fmt.Fprintln(os.Stderr, "\n  Modified:")
				for _, f := range modified {
					fmt.Fprintf(os.Stderr, "    %s\n", f)
				}
				fmt.Fprintln(os.Stderr, "\n  Clean up first, or re-run and choose \"None\" to skip git.")
				os.Exit(1)
			}
		}

		// Show full summary and confirm.
		tagPreview := config.FormatMessage(cfg.Git.TagPattern, newVer.String())
		confirmed, err := ui.Confirm(baseVer, newVer, gitAction, tagPreview, doPush)
		if err != nil {
			return err
		}
		if !confirmed {
			fmt.Println("Aborted.")
			return nil
		}
	} else {
		// ── Non-interactive path (bump type given as arg) ─────────────────────
		// Merge flags + config.
		doCommit = flagCommit || cfg.Git.Commit
		doTag = flagTag || cfg.Git.Tag
		doPush = flagPush || cfg.Git.Push
		if doTag {
			doCommit = true
		}
		if doPush {
			doCommit = true
		}
		// Dirty tree check before any writes.
		if doCommit && !flagDryRun {
			dirty, modified, gitErr := git.IsDirty()
			if gitErr != nil {
				return fmt.Errorf("checking git status: %w", gitErr)
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

		var tagName string
		if doTag {
			tagName = config.FormatMessage(cfg.Git.TagPattern, newVerStr)
			if err := git.Tag(tagName); err != nil {
				return err
			}
			fmt.Printf("✓ Tagged:    %s\n", tagName)
		}

		if doPush {
			fmt.Print("  Pushing…  ")
			if err := git.Push(tagName); err != nil {
				fmt.Println("✗ failed")
				fmt.Fprintf(os.Stderr, "\nError: %s\n", err)
				fmt.Fprintln(os.Stderr, "\nFiles were written and committed/tagged successfully.")
				fmt.Fprintln(os.Stderr, "Push failed — run manually:")
				if tagName != "" {
					fmt.Fprintf(os.Stderr, "  git push && git push origin %s\n", tagName)
				} else {
					fmt.Fprintln(os.Stderr, "  git push")
				}
				os.Exit(1)
			}
			fmt.Println("✓")
			fmt.Println("✓ Pushed")
		} else {
			// Push didn't happen — show manual instructions.
			if tagName != "" {
				fmt.Printf("\n  To push:\n    git push && git push origin %s\n", tagName)
			} else {
				fmt.Println("\n  To push:\n    git push")
			}
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
