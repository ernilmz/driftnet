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
