package main

import (
	"encoding/json"
	"errors"
	"net"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"time"
)

func TestAppBoundariesAndDefaultDirect(t *testing.T) {
	root := t.TempDir()
	app := filepath.Join(root, "Editor.app")
	if err := os.MkdirAll(filepath.Join(app, "Contents", "MacOS"), 0700); err != nil {
		t.Fatal(err)
	}
	app, err := filepath.EvalSymlinks(app)
	if err != nil {
		t.Fatal(err)
	}
	patterns, err := appPatterns([]App{{Path: app, Bundle: true}})
	if err != nil {
		t.Fatal(err)
	}
	matcher := regexp.MustCompile(patterns[0])
	if !matcher.MatchString(filepath.Join(app, "Contents", "MacOS", "Editor")) {
		t.Fatal("selected app did not match")
	}
	if matcher.MatchString(filepath.Join(root, "Editor.app-other", "Contents", "MacOS", "Editor")) {
		t.Fatal("sibling app was captured")
	}
	raw, err := routerOptions(Request{EnginePort: 12345, Apps: []App{{Path: app, Bundle: true}}})
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		Route struct {
			Final string `json:"final"`
		} `json:"route"`
		Inbounds []struct {
			DNSMode string `json:"dns_mode"`
		} `json:"inbounds"`
	}
	if err = json.Unmarshal(raw, &config); err != nil {
		t.Fatal(err)
	}
	if config.Route.Final != "direct" || config.Inbounds[0].DNSMode != "disabled" {
		t.Fatal("unmatched routing or system DNS was broadened")
	}
}
func TestInvalidLocalRequestIsRejectedBeforeStartup(t *testing.T) {
	if _, err := routerOptions(Request{EnginePort: 53}); err == nil {
		t.Fatal("privileged local endpoint accepted")
	}
	if _, err := appPatterns([]App{{Path: "relative/app.exe"}}); err == nil {
		t.Fatal("relative app accepted")
	}
	if _, err := appPatterns([]App{{Path: t.TempDir(), Bundle: true}}); err == nil {
		t.Fatal("non-app directory accepted")
	}
}

func TestIPv4UplinkDoesNotAdvertiseUnusableIPv6(t *testing.T) {
	addresses := tunnelAddresses(false)
	if len(addresses) != 1 || addresses[0] != "172.31.253.1/30" {
		t.Fatal("IPv4-only uplink exposed an IPv6 default route")
	}
	if len(tunnelAddresses(true)) != 2 {
		t.Fatal("Genuine dual-stack uplink lost IPv6 routing")
	}
}

// A real net.Conn with no receiver used to block forever while the handler
// retained its privileged worker and (for validation errors) session mutex.
func TestUnreadIPCReplyHasBoundedLifetime(t *testing.T) {
	server, client := net.Pipe()
	defer server.Close()
	defer client.Close()
	done := make(chan error, 1)
	go func() { done <- answer(server, false, errors.New(strings.Repeat("invalid path", 4096))) }()
	select {
	case err := <-done:
		if err == nil {
			t.Fatal("unread reply unexpectedly succeeded")
		}
		if timeout, ok := err.(net.Error); !ok || !timeout.Timeout() {
			t.Fatalf("want timeout, got %v", err)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("unread peer pinned a privileged worker")
	}
}

func TestPrevalidatedApplicationPathsAreNotReopened(t *testing.T) {
	app := filepath.Join(t.TempDir(), "Selected.exe")
	if err := os.WriteFile(app, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	patterns, err := appPatterns([]App{{Path: app}})
	if err != nil {
		t.Fatal(err)
	}
	if err = os.Remove(app); err != nil {
		t.Fatal(err)
	}
	// Privileged construction consumes only the user-validated strings. It
	// must not reopen request paths after leaving the impersonation scope.
	raw, err := routerOptionsWithPatterns(Request{EnginePort: 12345, Apps: []App{{Path: app}}}, patterns)
	if err != nil {
		t.Fatal(err)
	}
	var config struct {
		Route struct {
			Rules []struct {
				Paths []string `json:"process_path_regex"`
			} `json:"rules"`
		} `json:"route"`
	}
	if err = json.Unmarshal(raw, &config); err != nil {
		t.Fatal(err)
	}
	if len(config.Route.Rules) != 1 || len(config.Route.Rules[0].Paths) != 1 || config.Route.Rules[0].Paths[0] != patterns[0] {
		t.Fatal("validated patterns were replaced")
	}
}
