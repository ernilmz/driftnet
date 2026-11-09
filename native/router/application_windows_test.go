package main

import (
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
)

func TestWindowsCanonicalExecutableMatchesProcessMetadata(t *testing.T) {
	app := filepath.Join(t.TempDir(), "Selected.exe")
	if err := os.WriteFile(app, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	patterns, err := appPatterns([]App{{Path: `\\?\` + app}})
	if err != nil {
		t.Fatal(err)
	}
	matcher := regexp.MustCompile(patterns[0])
	metadata, err := filepath.EvalSymlinks(app)
	if err != nil {
		t.Fatal(err)
	}
	metadata = normalizeAppPath(metadata)
	if !matcher.MatchString(metadata) || !matcher.MatchString(strings.ToUpper(metadata)) {
		t.Fatalf("Rust canonical path did not match DOS process metadata: path=%q pattern=%q TEMP=%q", app, patterns[0], os.Getenv("TEMP"))
	}
	if matcher.MatchString(metadata + ".other") {
		t.Fatal("exact app rule widened")
	}
	if got := normalizeAppPath(`\\?\UNC\server\share\Editor.exe`); got != `\\server\share\Editor.exe` {
		t.Fatalf("UNC normalization: %q", got)
	}
}

func TestWindowsDiscordFamilyIsBoundedAndSurvivesUpdates(t *testing.T) {
	root := filepath.Join(t.TempDir(), "Discord")
	if err := os.Mkdir(root, 0700); err != nil {
		t.Fatal(err)
	}
	resolved, resolveErr := filepath.EvalSymlinks(root)
	t.Logf("family root=%q resolved=%q err=%v TEMP=%q", root, resolved, resolveErr, os.Getenv("TEMP"))
	if resolveErr != nil {
		t.Fatal(resolveErr)
	}
	// Model Rust canonicalize, which expands the runner's TEMP 8.3 alias.
	root = normalizeAppPath(resolved)
	patterns, err := appPatterns([]App{{Path: `\\?\` + root, Family: "discord"}})
	if err != nil {
		t.Fatal(err)
	}
	matcher := regexp.MustCompile(patterns[0])
	for _, relative := range []string{`Update.exe`, `app-1.0.0\Discord.exe`, `app-1.0.999\Discord.exe`, `app-2\Discord.exe`} {
		if !matcher.MatchString(filepath.Join(root, relative)) {
			t.Fatalf("family missed %s", relative)
		}
	}
	for _, path := range []string{
		filepath.Join(root+"-other", "Update.exe"), filepath.Join(filepath.Dir(root), "Other", "Update.exe"),
		filepath.Join(root, "app-1.0.0", "Other.exe"), filepath.Join(root, "app-1.0.0", "nested", "Discord.exe"),
		filepath.Join(root, "app-malicious", "Discord.exe"), filepath.Join(root, "app-1..0", "Discord.exe"),
		filepath.Join(root, "nested", "Update.exe"), filepath.Join(root, "Discord.exe"),
	} {
		if matcher.MatchString(path) {
			t.Fatalf("family captured unrelated path %s", path)
		}
	}
	if _, err := appPatterns([]App{{Path: root, Family: "anything"}}); err == nil {
		t.Fatal("unknown family accepted")
	}
	if _, err := appPatterns([]App{{Path: filepath.Dir(root), Family: "discord"}}); err == nil {
		t.Fatal("unbounded parent accepted")
	}
}

func TestWindowsDiscordFamilyRejectsRedirectedRoot(t *testing.T) {
	root := t.TempDir()
	target := filepath.Join(root, "actual", "Discord")
	link := filepath.Join(root, "Discord")
	if err := os.MkdirAll(target, 0700); err != nil {
		t.Fatal(err)
	}
	if output, err := exec.Command("cmd.exe", "/c", "mklink", "/J", link, target).CombinedOutput(); err != nil {
		t.Fatalf("junction fixture: %s: %v", output, err)
	}
	if _, err := appPatterns([]App{{Path: link, Family: "discord"}}); err == nil {
		t.Fatal("junction family root accepted")
	}
}
