package version

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
)

// WriteVersion writes the new semver string to the given file,
// preserving the file's existing formatting exactly (key order, whitespace, etc).
func WriteVersion(path, newVersion string) error {
	base := filepath.Base(path)
	ft, err := DetectType(base)
	if err != nil {
		return err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("reading %s: %w", path, err)
	}

	var newData []byte
	switch ft {
	case FileTypePackageJSON:
		newData, err = writePackageJSON(data, newVersion, path)
	case FileTypeCargoToml:
		newData, err = writeCargoToml(data, newVersion, path)
	case FileTypeVersionFile:
		newData, err = writeVersionFile(newVersion)
	default:
		return fmt.Errorf("unknown file type for %s", path)
	}
	if err != nil {
		return err
	}

	return os.WriteFile(path, newData, 0644)
}

// packageVersionRe matches the "version": "..." field at any indentation level.
var packageVersionRe = regexp.MustCompile(`"version"\s*:\s*"[^"]*"`)

// writePackageJSON replaces only the version value via regex, preserving all
// other formatting, key order, and whitespace exactly as-is.
func writePackageJSON(data []byte, newVersion, path string) ([]byte, error) {
	replacement := fmt.Sprintf(`"version": "%s"`, newVersion)
	result := packageVersionRe.ReplaceAll(data, []byte(replacement))
	if bytes.Equal(result, data) {
		return nil, fmt.Errorf("%s: could not find \"version\" field to update", path)
	}
	return result, nil
}

// writeCargoToml uses regex replacement so the rest of the TOML is untouched.
// Matches the first `version = "..."` line (Cargo convention: [package] first).
var cargoVersionRe = regexp.MustCompile(`(?m)^version\s*=\s*"[^"]*"`)

func writeCargoToml(data []byte, newVersion, path string) ([]byte, error) {
	replacement := fmt.Sprintf(`version = "%s"`, newVersion)
	result := cargoVersionRe.ReplaceAll(data, []byte(replacement))
	if bytes.Equal(result, data) {
		return nil, fmt.Errorf("%s: could not find version field to update", path)
	}
	return result, nil
}

func writeVersionFile(newVersion string) ([]byte, error) {
	return []byte(newVersion + "\n"), nil
}
