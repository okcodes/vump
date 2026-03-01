package version

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
)

// WriteVersion writes the new semver string to the given file,
// preserving the file's existing formatting as much as possible.
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

// writePackageJSON preserves JSON key ordering by doing a structured
// unmarshal → mutate → marshal with 2-space indent.
func writePackageJSON(data []byte, newVersion, path string) ([]byte, error) {
	// Use ordered map via json.RawMessage to preserve all fields.
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, fmt.Errorf("parsing %s: %w", path, err)
	}
	vBytes, err := json.Marshal(newVersion)
	if err != nil {
		return nil, err
	}
	raw["version"] = vBytes

	out, err := json.MarshalIndent(raw, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("marshalling %s: %w", path, err)
	}
	// json.MarshalIndent does not guarantee field order identical to input,
	// but preserves all keys. Append trailing newline.
	out = append(out, '\n')
	return out, nil
}

// writeCargoToml uses regex replacement so the rest of the TOML is untouched.
// Matches the `version = "..."` line inside [package] by finding the first
// occurrence of version = "..." in the file (Cargo convention: [package] first).
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
