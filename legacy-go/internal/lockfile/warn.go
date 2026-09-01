package lockfile

import (
	"fmt"
	"os"
	"path/filepath"
)

// CheckAndWarn prints warnings for stale lock files after version files are updated.
// It checks each updated file path and emits guidance if relevant lock files are found.
func CheckAndWarn(updatedPaths []string) {
	for _, p := range updatedPaths {
		base := filepath.Base(p)
		dir := filepath.Dir(p)

		switch base {
		case "package.json":
			warnNPM(dir, p)
		case "Cargo.toml":
			warnCargo(dir, p)
		}
	}
}

func warnNPM(dir, _ string) {
	candidates := []string{
		filepath.Join(dir, "package-lock.json"),
		filepath.Join(dir, "yarn.lock"),
		filepath.Join(dir, "pnpm-lock.yaml"),
	}
	for _, lock := range candidates {
		if _, err := os.Stat(lock); err == nil {
			lockName := filepath.Base(lock)
			cmd := "npm install"
			if lockName == "yarn.lock" {
				cmd = "yarn install"
			} else if lockName == "pnpm-lock.yaml" {
				cmd = "pnpm install"
			}
			fmt.Printf("\n⚠  Lock file out of date. Run the following to update it:\n")
			fmt.Printf("   cd %s && %s\n", dir, cmd)
			return
		}
	}
}

func warnCargo(dir, _ string) {
	// Check in the same directory first, then repo root (parent directories up to 3 levels).
	candidates := []string{
		filepath.Join(dir, "Cargo.lock"),
	}
	// Also check up to 3 parent dirs for workspace Cargo.lock.
	cur := dir
	for i := 0; i < 3; i++ {
		parent := filepath.Dir(cur)
		if parent == cur {
			break
		}
		candidates = append(candidates, filepath.Join(parent, "Cargo.lock"))
		cur = parent
	}

	for _, lock := range candidates {
		if _, err := os.Stat(lock); err == nil {
			fmt.Printf("\n⚠  Cargo.lock is out of date. Run the following to update it:\n")
			fmt.Printf("   cargo build\n")
			fmt.Printf("   (or any cargo command that triggers a lock file refresh)\n")
			return
		}
	}
}
