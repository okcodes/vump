package cmd

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"github.com/spf13/cobra"
)

const (
	githubAPI = "https://api.github.com/repos/okcodes/vump/releases/latest"
	oldSuffix = ".old" // used on Windows for safe rename-then-replace
)

var flagUpdateCheck bool

var updateCmd = &cobra.Command{
	Use:   "update",
	Short: "Update vump to the latest release from GitHub",
	Long: `update fetches the latest release from GitHub and replaces the running binary.

On macOS the universal (signed + notarized) binary is always downloaded.
On Linux and Windows the arch-specific binary is used.

Use --check to only print whether an update is available without downloading.`,
	Args:          cobra.NoArgs,
	RunE:          runUpdate,
	SilenceUsage:  true,
	SilenceErrors: true,
}

func init() {
	rootCmd.AddCommand(updateCmd)
	updateCmd.Flags().BoolVar(&flagUpdateCheck, "check", false, "Check for a new version without downloading")
}

// githubRelease is the subset of the GitHub releases API we need.
type githubRelease struct {
	TagName string `json:"tag_name"` // e.g. "v0.2.0"
}

func runUpdate(cmd *cobra.Command, _ []string) error {
	currentVersion := rootCmd.Version
	if currentVersion == "" {
		return fmt.Errorf("cannot update a dev build (version not embedded)")
	}

	// ── Fetch latest release ──────────────────────────────────────────────────
	fmt.Println("Checking for updates…")
	rel, err := fetchLatestRelease()
	if err != nil {
		return fmt.Errorf("checking GitHub releases: %w", err)
	}

	latestTag := rel.TagName                        // "v0.2.0"
	latestVer := strings.TrimPrefix(latestTag, "v") // "0.2.0"
	currentVer := strings.TrimPrefix(currentVersion, "v")

	if currentVer == latestVer {
		fmt.Printf("✓ Already up to date (%s)\n", latestTag)
		return nil
	}

	fmt.Printf("  Current:  %s\n", currentVersion)
	fmt.Printf("  Latest:   %s\n", latestTag)

	if flagUpdateCheck {
		fmt.Println("\nRun without --check to update.")
		os.Exit(1) // non-zero signals "update available" to callers/scripts
	}

	// ── Determine asset name for this platform ────────────────────────────────
	assetName := platformAsset()
	downloadURL := fmt.Sprintf(
		"https://github.com/okcodes/vump/releases/download/%s/%s",
		latestTag, assetName,
	)

	fmt.Printf("  Downloading %s…\n", assetName)

	// ── Download ──────────────────────────────────────────────────────────────
	execPath, err := os.Executable()
	if err != nil {
		return fmt.Errorf("locating current executable: %w", err)
	}
	execPath, err = filepath.EvalSymlinks(execPath)
	if err != nil {
		return fmt.Errorf("resolving symlinks: %w", err)
	}

	// Download into the same directory so rename is atomic (same filesystem).
	execDir := filepath.Dir(execPath)
	tmpFile, err := os.CreateTemp(execDir, "vump-update-*")
	if err != nil {
		return fmt.Errorf("creating temp file: %w", err)
	}
	tmpPath := tmpFile.Name()
	defer func() { os.Remove(tmpPath) }() // no-op if rename succeeded

	if err := downloadFile(tmpFile, downloadURL); err != nil {
		tmpFile.Close()
		return fmt.Errorf("downloading update: %w", err)
	}
	tmpFile.Close()

	if err := os.Chmod(tmpPath, 0755); err != nil {
		return fmt.Errorf("setting permissions: %w", err)
	}

	// ── Replace ───────────────────────────────────────────────────────────────
	if err := replaceExecutable(execPath, tmpPath); err != nil {
		return fmt.Errorf("replacing executable: %w", err)
	}

	fmt.Printf("✓ Updated to %s\n", latestTag)
	return nil
}

// platformAsset returns the GitHub release asset filename for the current OS/arch.
// On macOS we always use the universal binary (the only notarized one).
func platformAsset() string {
	switch runtime.GOOS {
	case "darwin":
		return "vump-darwin-universal"
	case "windows":
		return fmt.Sprintf("vump-windows-%s.exe", runtime.GOARCH)
	default: // linux and anything else
		return fmt.Sprintf("vump-%s-%s", runtime.GOOS, runtime.GOARCH)
	}
}

func fetchLatestRelease() (*githubRelease, error) {
	req, err := http.NewRequest(http.MethodGet, githubAPI, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "vump-cli")

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("GitHub API returned %s", resp.Status)
	}

	var rel githubRelease
	if err := json.NewDecoder(resp.Body).Decode(&rel); err != nil {
		return nil, fmt.Errorf("parsing release JSON: %w", err)
	}
	if rel.TagName == "" {
		return nil, fmt.Errorf("no tag_name in GitHub response")
	}
	return &rel, nil
}

func downloadFile(dst *os.File, url string) error {
	resp, err := http.Get(url) //nolint:noctx
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("download returned %s", resp.Status)
	}

	_, err = io.Copy(dst, resp.Body)
	return err
}

// replaceExecutable swaps newPath over execPath.
//
// On Unix: atomic os.Rename works even while the binary is running because
// the OS tracks open files by inode, not path.
//
// On Windows: a running .exe cannot be overwritten, but it *can* be renamed.
// We rename it to execPath+".old" first, then rename the new binary into place.
// The .old file is cleaned up on the next invocation.
func replaceExecutable(execPath, newPath string) error {
	if runtime.GOOS == "windows" {
		return replaceWindows(execPath, newPath)
	}
	return os.Rename(newPath, execPath)
}

func replaceWindows(execPath, newPath string) error {
	oldPath := execPath + oldSuffix

	// Clean up any leftover .old from a previous update attempt.
	os.Remove(oldPath)

	// Step 1: Move the running binary aside.
	if err := os.Rename(execPath, oldPath); err != nil {
		return fmt.Errorf("renaming current binary: %w", err)
	}

	// Step 2: Move the new binary into place.
	if err := os.Rename(newPath, execPath); err != nil {
		// Recovery: try to put the old binary back.
		os.Rename(oldPath, execPath) //nolint:errcheck
		return fmt.Errorf("placing new binary: %w", err)
	}

	// Step 3: Remove the old binary (best-effort; may fail if locked).
	os.Remove(oldPath)
	return nil
}
