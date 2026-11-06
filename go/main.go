package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"os/signal"
	"path/filepath"
	"sort"
	"strings"
	"syscall"

	"github.com/dreygur/cleaner/internal/clean"
	"github.com/dreygur/cleaner/internal/config"
	"github.com/dreygur/cleaner/internal/models"
	"github.com/dreygur/cleaner/internal/scan"
	"github.com/dreygur/cleaner/internal/utils"
	"github.com/dreygur/cleaner/internal/volumes"
)

const appName = "cleaner"

// appVersion is set with -ldflags "-X main.appVersion=..." at build time.
// The Makefile fills it from `git describe`; a `go build` without that flag
// leaves it at "dev".
var appVersion = "dev"

// minReportSize hides groups too small to be worth a decision.
const minReportSize = 1 << 20

func homeDir() string {
	home, err := os.UserHomeDir()
	if err != nil {
		utils.PrintLog("cannot resolve home directory: "+err.Error(), utils.LevelError)
		os.Exit(1)
	}
	return home
}

func freeBytes(path string) int64 {
	var stat syscall.Statfs_t
	if err := syscall.Statfs(path, &stat); err != nil {
		utils.PrintLog("cannot read free space for "+path+": "+err.Error(), utils.LevelWarn)
		return 0
	}
	return int64(stat.Bavail) * int64(stat.Bsize)
}

func parseList(raw string) map[string]bool {
	set := map[string]bool{}
	for _, item := range strings.Split(raw, ",") {
		if item = strings.TrimSpace(item); item != "" {
			set[item] = true
		}
	}
	return set
}

func unknownIDs(all []models.Rule, names map[string]bool) []string {
	known := make(map[string]bool, len(all))
	for _, rule := range all {
		known[rule.ID] = true
	}
	var unknown []string
	for name := range names {
		if !known[name] {
			unknown = append(unknown, name)
		}
	}
	sort.Strings(unknown)
	return unknown
}

func checkIDs(all []models.Rule, flagName string, names map[string]bool) {
	unknown := unknownIDs(all, names)
	if len(unknown) == 0 {
		return
	}
	var ids []string
	for _, rule := range all {
		ids = append(ids, rule.ID)
	}
	sort.Strings(ids)
	utils.PrintLog(fmt.Sprintf("unknown rule id in --%s: %s (known: %s)",
		flagName, strings.Join(unknown, ", "), strings.Join(ids, ", ")), utils.LevelError)
	os.Exit(1)
}

func selectRules(all []models.Rule, only, skip map[string]bool) []models.Rule {
	var chosen []models.Rule
	for _, rule := range all {
		if len(only) > 0 && !only[rule.ID] {
			continue
		}
		if skip[rule.ID] {
			continue
		}
		chosen = append(chosen, rule)
	}
	return chosen
}

// resolveRoots defaults to the whole home directory plus every attached
// local disk, so projects outside a conventional folder are still found.
func resolveRoots(home string, mounts []string, raw string) []string {
	if raw == "" {
		return append([]string{home}, mounts...)
	}
	var roots []string
	for _, r := range strings.Split(raw, ",") {
		if r = strings.TrimSpace(r); r != "" {
			roots = append(roots, r)
		}
	}
	return roots
}

func confirm(in *bufio.Reader, question string) bool {
	fmt.Printf("    %s [y/N] ", question)
	answer, err := in.ReadString('\n')
	if err != nil {
		return false
	}
	answer = strings.ToLower(strings.TrimSpace(answer))
	return answer == "y" || answer == "yes"
}

// displayPath shortens paths under home to ~/..., and leaves paths on other
// disks absolute.
func displayPath(home, path string) string {
	rel, err := filepath.Rel(home, path)
	if err != nil || strings.HasPrefix(rel, "..") {
		return path
	}
	return "~/" + rel
}

func report(groups []models.Group, home string) int64 {
	var total int64
	for _, g := range groups {
		total += g.Size
		fmt.Printf("  %-16s %10s  %s\n", g.Rule.ID, utils.HumanSize(g.Size), g.Rule.Description)
		for _, t := range g.Targets {
			if t.Size < minReportSize {
				continue
			}
			fmt.Printf("  %-16s %10s    %s\n", "", utils.HumanSize(t.Size), displayPath(home, t.Path))
		}
		fmt.Printf("  %-16s %10s    regenerate with: %s\n\n", "", "", g.Rule.Regen)
	}
	return total
}

func listRules(rules []models.Rule) {
	sorted := append([]models.Rule(nil), rules...)
	sort.Slice(sorted, func(i, j int) bool { return sorted[i].ID < sorted[j].ID })
	for _, r := range sorted {
		fmt.Printf("  %-16s %-40s regenerate with: %s\n", r.ID, r.Description, r.Regen)
	}
}

func sweep(scope clean.Scope, groups []models.Group, assumeYes bool, interrupted <-chan os.Signal) int64 {
	in := bufio.NewReader(os.Stdin)
	var freed int64

	for _, g := range groups {
		select {
		case <-interrupted:
			utils.PrintLog("interrupted, stopping before "+g.Rule.ID, utils.LevelWarn)
			return freed
		default:
		}
		if !assumeYes && !confirm(in, fmt.Sprintf("delete %s (%s)?", g.Rule.ID, utils.HumanSize(g.Size))) {
			utils.PrintLog("skipped "+g.Rule.ID, utils.LevelWarn)
			continue
		}
		got, failures := clean.Group(scope, g)
		freed += got
		for _, err := range failures {
			utils.PrintLog(err.Error(), utils.LevelError)
		}
		utils.PrintLog(fmt.Sprintf("%s freed %s", g.Rule.ID, utils.HumanSize(got)), utils.LevelInfo)
	}
	return freed
}

func main() {
	var (
		assumeYes = flag.Bool("yes", false, "delete every matched group without prompting")
		dryRun    = flag.Bool("dry-run", false, "report what would be deleted, then exit")
		only      = flag.String("only", "", "comma-separated rule ids to run exclusively")
		skip      = flag.String("skip", "", "comma-separated rule ids to leave alone")
		roots     = flag.String("roots", "", "comma-separated directories to search for build artifacts")
		version   = flag.Bool("version", false, "print version and exit")
		listOnly  = flag.Bool("list-rules", false, "print known rule ids and what they remove, then exit")
	)
	flag.Parse()

	if *version {
		fmt.Println(appName, appVersion)
		return
	}

	home := homeDir()
	mounts := volumes.List(volumes.MountDir)
	all := config.Rules(home, resolveRoots(home, mounts, *roots))

	if *listOnly {
		listRules(all)
		return
	}

	onlyIDs, skipIDs := parseList(*only), parseList(*skip)
	checkIDs(all, "only", onlyIDs)
	checkIDs(all, "skip", skipIDs)

	rules := selectRules(all, onlyIDs, skipIDs)
	if len(rules) == 0 {
		utils.PrintLog("no rules selected", utils.LevelWarn)
		return
	}

	before := freeBytes(home)
	utils.PrintLog(fmt.Sprintf("scanning %d rules, %s free", len(rules), utils.HumanSize(before)), utils.LevelInfo)
	for _, m := range mounts {
		utils.PrintLog("including disk "+m, utils.LevelInfo)
	}

	groups := scan.Scan(rules)
	sort.Slice(groups, func(i, j int) bool { return groups[i].Size > groups[j].Size })
	if len(groups) == 0 {
		utils.PrintLog("nothing to clean", utils.LevelInfo)
		return
	}

	fmt.Println()
	total := report(groups, home)
	utils.PrintLog(fmt.Sprintf("reclaimable: %s", utils.HumanSize(total)), utils.LevelInfo)

	if *dryRun {
		return
	}

	interrupted := make(chan os.Signal, 1)
	signal.Notify(interrupted, os.Interrupt)

	freed := sweep(clean.Scope{Home: home, Volumes: mounts}, groups, *assumeYes, interrupted)
	utils.PrintLog(fmt.Sprintf("freed %s, now %s free", utils.HumanSize(freed), utils.HumanSize(freeBytes(home))), utils.LevelInfo)
}
