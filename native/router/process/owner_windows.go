package process

import (
	"github.com/sagernet/sing-box/adapter"
	"golang.org/x/sys/windows"
	"path/filepath"
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
	if windows.OpenProcessToken(p, windows.TOKEN_QUERY, &token) != nil {
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
	path, err := driftCanonicalProcessPath(info.ProcessPath)
	if err != nil {
		return false
	}
	info.ProcessPath = path
	return true
}

func driftCanonicalProcessPath(path string) (string, error) {
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		return "", err
	}
	if strings.HasPrefix(strings.ToUpper(resolved), `\\?\UNC\`) {
		resolved = `\\` + resolved[8:]
	} else {
		resolved = strings.TrimPrefix(resolved, `\\?\`)
	}
	return filepath.Clean(resolved), nil
}
