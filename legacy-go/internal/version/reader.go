package version

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/pelletier/go-toml/v2"
)

// ReadVersion reads the semver string from the given file.
func ReadVersion(path string) (string, error) {
	base := filepath.Base(path)
	ft, err := DetectType(base)
	if err != nil {
		return "", err
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("reading %s: %w", path, err)
	}

	switch ft {
	case FileTypePackageJSON:
		return readPackageJSON(data, path)
	case FileTypeCargoToml:
		return readCargoToml(data, path)
	case FileTypeVersionFile:
		return readVersionFile(data, path)
	}
	return "", fmt.Errorf("unknown file type for %s", path)
}

func readPackageJSON(data []byte, path string) (string, error) {
	var pkg struct {
		Version string `json:"version"`
	}
	if err := json.Unmarshal(data, &pkg); err != nil {
		return "", fmt.Errorf("parsing %s as JSON: %w", path, err)
	}
	if pkg.Version == "" {
		return "", fmt.Errorf("%s has no \"version\" field", path)
	}
	return pkg.Version, nil
}

func readCargoToml(data []byte, path string) (string, error) {
	var cargo struct {
		Package struct {
			Version string `toml:"version"`
		} `toml:"package"`
	}
	if err := toml.Unmarshal(data, &cargo); err != nil {
		return "", fmt.Errorf("parsing %s as TOML: %w", path, err)
	}
	if cargo.Package.Version == "" {
		return "", fmt.Errorf("%s has no [package].version field", path)
	}
	return cargo.Package.Version, nil
}

func readVersionFile(data []byte, path string) (string, error) {
	v := strings.TrimSpace(string(data))
	if v == "" {
		return "", fmt.Errorf("%s is empty", path)
	}
	return v, nil
}
