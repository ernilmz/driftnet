// Copied into the patched upstream common/process package by the build script.
package process

import (
	"github.com/sagernet/sing-box/adapter"
	"strconv"
)

func driftOwnerAllowed(info *adapter.ConnectionOwner) bool {
	owner := driftExpectedOwner()
	return owner == "" || (info.UserId >= 0 && strconv.FormatInt(int64(info.UserId), 10) == owner)
}
