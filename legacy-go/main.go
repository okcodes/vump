package main

import "github.com/okcodes/vump/cmd"

// Version is injected at build time via:
//
//	-ldflags "-X main.Version=<version>"
//
// When built locally without ldflags it defaults to "dev".
var Version = "dev"

func main() {
	cmd.Execute(Version)
}
