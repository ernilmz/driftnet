// Copied into the patched upstream common/process package by the build script.
package process

import (
	"encoding/binary"
	"github.com/sagernet/sing-box/adapter"
	"strconv"
	"syscall"
	"unsafe"
)

func driftOwnerAllowed(info *adapter.ConnectionOwner) bool {
	owner := driftExpectedOwner()
	return owner == "" || (info.UserId >= 0 && strconv.FormatInt(int64(info.UserId), 10) == owner)
}

// Read the kernel's effective uid for a delegated socket's process. The BSD
// layout is checked by returned size and pid before using the uid field.
func driftProcessUID(pid uint32) (int32, error) {
	b := make([]byte, 256)
	size, _, err := syscall.Syscall6(syscall.SYS_PROC_INFO, 2, uintptr(pid), 3, 0, uintptr(unsafe.Pointer(&b[0])), uintptr(len(b)))
	if err != 0 {
		return -1, err
	}
	if size < 24 || binary.NativeEndian.Uint32(b[12:16]) != pid {
		return -1, syscall.EINVAL
	}
	return int32(binary.NativeEndian.Uint32(b[20:24])), nil
}
