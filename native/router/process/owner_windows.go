package process

import (
	"github.com/sagernet/sing-box/adapter"
	"golang.org/x/sys/windows"
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
	return err == nil && u.User.Sid.String() == owner
}
