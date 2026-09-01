package version

import "fmt"

// FileType is an enum for the supported version file formats.
type FileType int

const (
	FileTypePackageJSON FileType = iota
	FileTypeCargoToml
	FileTypeVersionFile
)

// DetectType returns the FileType based on the base filename alone.
// Returns an error for any unsupported filename.
func DetectType(basename string) (FileType, error) {
	switch basename {
	case "package.json":
		return FileTypePackageJSON, nil
	case "Cargo.toml":
		return FileTypeCargoToml, nil
	case "VERSION":
		return FileTypeVersionFile, nil
	default:
		return 0, fmt.Errorf(
			"unsupported file %q — vump only understands package.json, Cargo.toml, and VERSION",
			basename,
		)
	}
}
