package models

// Kind selects how a Rule locates the paths it is responsible for.
type Kind string

const (
	// KindArtifact walks each search root for directories named Dir that sit
	// beside a Marker file, so a build output is never confused with an
	// unrelated source folder that happens to share the name.
	KindArtifact Kind = "artifact"

	// KindFixed names a single absolute directory whose children are removed.
	// The directory itself survives so tools that expect it keep working.
	KindFixed Kind = "fixed"
)

// Rule describes one class of reclaimable disk space.
type Rule struct {
	// ID is the stable slug used on the command line to select this rule.
	ID string

	// Description is the one-line summary shown in the scan report.
	Description string

	// Kind decides which of the locating fields below apply.
	Kind Kind

	// Dir is the directory name matched by a KindArtifact rule.
	Dir string

	// Marker is a file that must exist next to Dir for a match to count.
	// An empty Marker matches on directory name alone.
	Marker string

	// Roots are the directories a KindArtifact rule walks.
	Roots []string

	// Paths are the absolute directories emptied by a KindFixed rule.
	Paths []string

	// Regen tells the user how the deleted data comes back, so the report
	// carries its own undo instructions.
	Regen string
}

// Target is one concrete directory matched by a Rule, with its measured size.
type Target struct {
	Path string
	Size int64
}

// Group collects every Target found for a single Rule.
type Group struct {
	Rule    Rule
	Targets []Target
	Size    int64
}
