package git

import (
	"bytes"
	"fmt"
	"os/exec"
	"strings"
)

// IsDirty returns true if there are uncommitted changes in the working tree,
// along with the list of modified files.
func IsDirty() (bool, []string, error) {
	cmd := exec.Command("git", "status", "--porcelain")
	out, err := cmd.Output()
	if err != nil {
		return false, nil, fmt.Errorf("git status failed: %w", err)
	}
	if len(bytes.TrimSpace(out)) == 0 {
		return false, nil, nil
	}
	lines := strings.Split(strings.TrimRight(string(out), "\n"), "\n")
	var files []string
	for _, l := range lines {
		if len(l) > 3 {
			files = append(files, strings.TrimSpace(l[3:]))
		}
	}
	return true, files, nil
}

// AddFiles stages each file for commit.
func AddFiles(paths []string) error {
	args := append([]string{"add", "--"}, paths...)
	cmd := exec.Command("git", args...)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git add failed: %s", strings.TrimSpace(string(out)))
	}
	return nil
}

// Commit creates a commit with the given message.
func Commit(message string) error {
	cmd := exec.Command("git", "commit", "-m", message)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git commit failed: %s", strings.TrimSpace(string(out)))
	}
	return nil
}

// Tag creates a lightweight tag at HEAD.
func Tag(name string) error {
	cmd := exec.Command("git", "tag", name)
	out, err := cmd.CombinedOutput()
	if err != nil {
		return fmt.Errorf("git tag failed: %s", strings.TrimSpace(string(out)))
	}
	return nil
}
