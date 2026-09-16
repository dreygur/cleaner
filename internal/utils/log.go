package utils

import (
	"fmt"

	"github.com/fatih/color"
)

const (
	// LevelInfo marks a completed step.
	LevelInfo = "info"
	// LevelError marks a failure the caller could not recover from.
	LevelError = "error"
	// LevelWarn marks a recoverable problem worth surfacing.
	LevelWarn = "warn"
)

const (
	unitStep  = 1024
	unitNames = "KMGT"
)

// HumanSize renders a byte count in binary units, matching what `du -h` prints.
func HumanSize(b int64) string {
	if b < unitStep {
		return fmt.Sprintf("%d B", b)
	}
	div, exp := int64(unitStep), 0
	for n := b / unitStep; n >= unitStep; n /= unitStep {
		div *= unitStep
		exp++
	}
	return fmt.Sprintf("%.1f %ciB", float64(b)/float64(div), unitNames[exp])
}

// PrintLog writes a status-prefixed, colored line to stdout.
func PrintLog(text, status string) {
	switch status {
	case LevelInfo:
		color.New(color.FgGreen).Println("[+] " + text)
	case LevelError:
		color.New(color.FgRed).Println("[-] " + text)
	case LevelWarn:
		color.New(color.FgYellow).Println("[!] " + text)
	default:
		color.New(color.FgCyan).Println("[*] " + text)
	}
}
