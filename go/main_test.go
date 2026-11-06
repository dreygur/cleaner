package main

import "testing"

func TestDisplayPath(t *testing.T) {
	cases := map[string]string{
		"/Users/me/Code/app/target":   "~/Code/app/target",
		"/Volumes/Disk/proj/target":   "/Volumes/Disk/proj/target",
		"/Users/meow/Code/app/target": "/Users/meow/Code/app/target",
	}
	for path, want := range cases {
		if got := displayPath("/Users/me", path); got != want {
			t.Errorf("displayPath(%q) = %q, want %q", path, got, want)
		}
	}
}
