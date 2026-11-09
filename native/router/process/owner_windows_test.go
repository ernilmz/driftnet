package process

import (
	"github.com/sagernet/sing-box/adapter"
	"golang.org/x/sys/windows"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestWindowsScopePreservesOwnerAndNormalizesProcessPath(t *testing.T) {
	token := windows.GetCurrentProcessToken()
	user, err := token.GetTokenUser()
	if err != nil {
		t.Fatal(err)
	}
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	canonical, err := filepath.EvalSymlinks(executable)
	if err != nil {
		t.Fatal(err)
	}
	if err = SetDriftScope(user.User.Sid.String(), nil); err != nil {
		t.Fatal(err)
	}
	defer driftScope.Store(nil)
	info := &adapter.ConnectionOwner{ProcessID: uint32(os.Getpid()), ProcessPath: `\\?\` + strings.TrimPrefix(canonical, `\\?\`)}
	if !driftOwnerAllowed(info) || strings.HasPrefix(info.ProcessPath, `\\?\`) {
		t.Fatal("owned process was not normalized")
	}
	if err = SetDriftScope("S-1-0-0", nil); err != nil {
		t.Fatal(err)
	}
	if driftOwnerAllowed(info) {
		t.Fatal("foreign SID became eligible for selected routing")
	}
}

func TestWindowsProcessJunctionResolvesOutsideSelectedFamily(t *testing.T) {
	root := t.TempDir()
	outside := filepath.Join(root, "Other")
	version := filepath.Join(root, "Discord", "app-9.9.9")
	if err := os.MkdirAll(outside, 0700); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Dir(version), 0700); err != nil {
		t.Fatal(err)
	}
	executable := filepath.Join(outside, "Discord.exe")
	if err := os.WriteFile(executable, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	if output, err := exec.Command("cmd.exe", "/c", "mklink", "/J", version, outside).CombinedOutput(); err != nil {
		t.Fatalf("junction fixture: %s: %v", output, err)
	}
	linkTarget, linkErr := os.Readlink(version)
	_, directErr := os.Stat(executable)
	_, throughErr := os.Stat(filepath.Join(version, "Discord.exe"))
	t.Logf("junction path=%q target=%q readlink=%v directStat=%v throughStat=%v TEMP=%q", version, linkTarget, linkErr, directErr, throughErr, os.Getenv("TEMP"))
	resolved, err := driftCanonicalProcessPath(filepath.Join(version, "Discord.exe"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.EqualFold(resolved, executable) {
		t.Fatalf("junction stayed inside apparent family: %q", resolved)
	}
}
