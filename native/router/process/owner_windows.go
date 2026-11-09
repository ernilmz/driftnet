package process

import (
	"github.com/sagernet/sing-box/adapter"
	"golang.org/x/sys/windows"
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

func driftOwnerAllowed(info *adapter.ConnectionOwner) bool {
	owner := driftExpectedOwner()
	if owner == "" {
		return true
	}
	p, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION, false, info.ProcessID)
	if err != nil {
		return false
	}
	defer windows.CloseHandle(p)
	var token windows.Token
	if windows.OpenProcessToken(p, windows.TOKEN_QUERY|windows.TOKEN_DUPLICATE, &token) != nil {
		return false
	}
	defer token.Close()
	u, err := token.GetTokenUser()
	if err != nil || u.User.Sid.String() != owner {
		return false
	}
	// QueryFullProcessImageName and Rust canonicalize spell the same path
	// differently. Resolve reparse points before the bounded family regex too:
	// a future version directory must not redirect matching outside its root.
	path, err := driftAsProcessUser(token, func() (string, error) {
		return driftCanonicalProcessPath(info.ProcessPath)
	})
	if err != nil {
		return false
	}
	info.ProcessPath = path
	return true
}

// Filesystem resolution can traverse a user-controlled junction or UNC path.
// Never perform that access using the service's SYSTEM network credentials.
// The caller must verify this process token's SID before entering this scope.
func driftAsProcessUser(token windows.Token, action func() (string, error)) (string, error) {
	var path string
	err := DriftWithProcessUser(token, func() (err error) {
		path, err = action()
		return err
	})
	return path, err
}

// DriftWithProcessUser scopes filesystem validation to an already SID-verified
// process token, independent of an IPC client's identification-only SQOS token.
func DriftWithProcessUser(token windows.Token, action func() error) error {
	var impersonation windows.Token
	if err := windows.DuplicateTokenEx(token, windows.TOKEN_IMPERSONATE|windows.TOKEN_QUERY,
		nil, windows.SecurityImpersonation, windows.TokenImpersonation, &impersonation); err != nil {
		return err
	}
	defer impersonation.Close()
	return DriftWithImpersonation(func() error { return windows.SetThreadToken(nil, impersonation) }, action)
}

// DriftWithImpersonation keeps the thread pinned until its original service
// identity is restored. Used only for bounded token inspection/path validation.
func DriftWithImpersonation(impersonate func() error, action func() error) error {
	runtime.LockOSThread()
	if err := impersonate(); err != nil {
		runtime.UnlockOSThread()
		return err
	}
	defer func() {
		if windows.RevertToSelf() != nil {
			// Microsoft requires fail-stop on reversion failure. Do not ever
			// return an impersonated OS thread to Go's scheduler or pool.
			_, _ = os.Stderr.WriteString("Driftnet: thread security restoration failed\n")
			os.Exit(1)
		}
		runtime.UnlockOSThread()
	}()
	return action()
}

func driftCanonicalProcessPath(path string) (string, error) {
	encoded, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return "", err
	}
	// Resolve the opened image's real DOS name in the OS, like Rust's
	// canonicalize. Go's component walk can fail for valid junctions whose
	// targets contain short 8.3 aliases, even when opening the file succeeds.
	file, err := windows.CreateFile(encoded, windows.FILE_READ_ATTRIBUTES,
		windows.FILE_SHARE_READ|windows.FILE_SHARE_WRITE|windows.FILE_SHARE_DELETE,
		nil, windows.OPEN_EXISTING, windows.FILE_FLAG_BACKUP_SEMANTICS, 0)
	if err != nil {
		return "", err
	}
	defer windows.CloseHandle(file)
	buffer := make([]uint16, 512)
	for {
		size, err := windows.GetFinalPathNameByHandle(file, &buffer[0], uint32(len(buffer)), 0)
		if err != nil {
			return "", err
		}
		if size == 0 || size > 32767 {
			return "", windows.ERROR_INVALID_DATA
		}
		if size >= uint32(len(buffer)) {
			buffer = make([]uint16, size+1)
			continue
		}
		resolved := windows.UTF16ToString(buffer[:size])
		if strings.HasPrefix(strings.ToUpper(resolved), `\\?\UNC\`) {
			resolved = `\\` + resolved[8:]
		} else {
			resolved = strings.TrimPrefix(resolved, `\\?\`)
		}
		return filepath.Clean(resolved), nil
	}
}
