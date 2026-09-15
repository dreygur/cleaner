package main

import (
	"bufio"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"syscall"

	"github.com/dreygur/cleaner/internal/clean"
	"github.com/dreygur/cleaner/internal/config"
	"github.com/dreygur/cleaner/internal/models"
	"github.com/dreygur/cleaner/internal/scan"
	"github.com/dreygur/cleaner/internal/utils"
)

const (
	appName    = "cleaner"
	appVersion = "v0.1.0"
)

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

func resolveRoots(home, raw string) []string {
	if raw != "" {
		var roots []string
		for _, r := range strings.Split(raw, ",") {
			if r = strings.TrimSpace(r); r != "" {
				roots = append(roots, r)
			}
		}
		return roots
	}
	var roots []string
	for _, name := range config.DefaultSearchRoots {
		roots = append(roots, filepath.Join(home, name))
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

func report(groups []models.Group, home string) int64 {
	var total int64
	for _, g := range groups {
		total += g.Size
		fmt.Printf("  %-16s %10s  %s\n", g.Rule.ID, utils.HumanSize(g.Size), g.Rule.Description)
		for _, t := range g.Targets {
			if t.Size < minReportSize {
				continue
			}
			rel, err := filepath.Rel(home, t.Path)
			if err != nil {
				rel = t.Path
			}
			fmt.Printf("  %-16s %10s    ~/%s\n", "", utils.HumanSize(t.Size), rel)
		}
		fmt.Printf("  %-16s %10s    regenerate with: %s\n\n", "", "", g.Rule.Regen)
	}
	return total
}

func sweep(home string, groups []models.Group, assumeYes bool) int64 {
	in := bufio.NewReader(os.Stdin)
	var freed int64

	for _, g := range groups {
		if !assumeYes && !confirm(in, fmt.Sprintf("delete %s (%s)?", g.Rule.ID, utils.HumanSize(g.Size))) {
			utils.PrintLog("skipped "+g.Rule.ID, utils.LevelWarn)
			continue
		}
		got, failures := clean.Group(home, g)
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
	)
	flag.Parse()

	if *version {
		fmt.Println(appName, appVersion)
		return
	}

	home := homeDir()
	rules := selectRules(config.Rules(home, resolveRoots(home, *roots)), parseList(*only), parseList(*skip))
	if len(rules) == 0 {
		utils.PrintLog("no rules selected", utils.LevelWarn)
		return
	}

	before := freeBytes(home)
	utils.PrintLog(fmt.Sprintf("scanning %d rules, %s free", len(rules), utils.HumanSize(before)), utils.LevelInfo)

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

	freed := sweep(home, groups, *assumeYes)
	utils.PrintLog(fmt.Sprintf("freed %s, now %s free", utils.HumanSize(freed), utils.HumanSize(freeBytes(home))), utils.LevelInfo)
}
