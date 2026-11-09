package process

import (
	"github.com/sagernet/sing-box/adapter"
	"golang.org/x/sys/windows"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
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
	canonicalOutside, err := driftCanonicalProcessPath(executable)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.EqualFold(resolved, canonicalOutside) {
		t.Fatalf("junction stayed inside apparent family: got %q want %q", resolved, canonicalOutside)
	}
}

func TestWindowsPathResolutionImpersonatesProcessUserAndReverts(t *testing.T) {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	var initial windows.Token
	if err := windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &initial); err != windows.ERROR_NO_TOKEN {
		if err == nil {
			initial.Close()
		}
		t.Fatalf("fixture thread did not start without impersonation: %v", err)
	}
	var token windows.Token
	if err := windows.OpenProcessToken(windows.CurrentProcess(), windows.TOKEN_QUERY|windows.TOKEN_DUPLICATE, &token); err != nil {
		t.Fatal(err)
	}
	defer token.Close()
	owner, err := token.GetTokenUser()
	if err != nil {
		t.Fatal(err)
	}
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	observed := false
	resolved, err := driftAsProcessUser(token, func() (string, error) {
		var effective windows.Token
		if err := windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &effective); err != nil {
			return "", err
		}
		defer effective.Close()
		user, err := effective.GetTokenUser()
		if err != nil {
			return "", err
		}
		if user.User.Sid.String() != owner.User.Sid.String() {
			return "", windows.ERROR_ACCESS_DENIED
		}
		observed = true
		return driftCanonicalProcessPath(executable)
	})
	if err != nil {
		t.Fatal(err)
	}
	if !observed || resolved == "" {
		t.Fatal("canonicalization did not run under the process user's impersonation token")
	}
	var after windows.Token
	if err := windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &after); err != windows.ERROR_NO_TOKEN {
		if err == nil {
			after.Close()
		}
		t.Fatalf("filesystem scope left the thread impersonating: %v", err)
	}
	// Path errors must run the same reversion guard.
	if _, err := driftAsProcessUser(token, func() (string, error) { return driftCanonicalProcessPath(executable + ".missing") }); err == nil {
		t.Fatal("missing image path unexpectedly resolved")
	}
	if err := windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &after); err != windows.ERROR_NO_TOKEN {
		if err == nil {
			after.Close()
		}
		t.Fatalf("failed filesystem operation left thread impersonating: %v", err)
	}
	called := false
	if _, err := driftAsProcessUser(0, func() (string, error) { called = true; return "", nil }); err == nil || called {
		t.Fatal("token duplication failure did not fail closed")
	}
}
