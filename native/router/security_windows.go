package main

import (
	"errors"
	"fmt"
	"path/filepath"
	"strings"
	"unsafe"

	"golang.org/x/sys/windows"
)

const trustedInstallerSID = "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464"

func trustedWindowsOwner(sid string) bool {
	return sid == "S-1-5-18" || sid == "S-1-5-32-544" || sid == trustedInstallerSID
}

func validateWindowsDescriptor(descriptor *windows.SECURITY_DESCRIPTOR) error {
	owner, _, err := descriptor.Owner()
	if err != nil || owner == nil || !trustedWindowsOwner(owner.String()) {
		return errors.New("Networking files must be owned by SYSTEM or Administrators")
	}
	dacl, _, err := descriptor.DACL()
	if err != nil || dacl == nil {
		return errors.New("Networking files require a restrictive access control list")
	}
	// Write data/append/EA/attributes, delete children/object, change ACL/owner,
	// and generic write/all can each let an ordinary user replace SYSTEM code.
	const unsafeAccess = 0x000d0156 | 0x40000000 | 0x10000000
	for i := uint32(0); i < uint32(dacl.AceCount); i++ {
		var ace *windows.ACCESS_ALLOWED_ACE
		if err := windows.GetAce(dacl, i, &ace); err != nil {
			return err
		}
		if ace.Header.AceFlags&windows.INHERIT_ONLY_ACE != 0 || ace.Header.AceType == windows.ACCESS_DENIED_ACE_TYPE {
			continue
		}
		if ace.Header.AceType != windows.ACCESS_ALLOWED_ACE_TYPE {
			return errors.New("Unsupported networking file access control entry")
		}
		sid := (*windows.SID)(unsafe.Pointer(&ace.SidStart))
		if uint32(ace.Mask)&unsafeAccess != 0 && !trustedWindowsOwner(sid.String()) {
			return errors.New("Networking files are writable by an unprivileged account")
		}
	}
	return nil
}

func validateWindowsFile(path string) error {
	name, err := windows.UTF16PtrFromString(path)
	if err != nil {
		return err
	}
	attributes, err := windows.GetFileAttributes(name)
	if err != nil {
		return err
	}
	if attributes&windows.FILE_ATTRIBUTE_REPARSE_POINT != 0 {
		return errors.New("Networking installation must not contain reparse points")
	}
	descriptor, err := windows.GetNamedSecurityInfo(path, windows.SE_FILE_OBJECT, windows.OWNER_SECURITY_INFORMATION|windows.DACL_SECURITY_INFORMATION)
	if err != nil {
		return err
	}
	return validateWindowsDescriptor(descriptor)
}

func validateWindowsServiceExecutable(executable string) error {
	root, err := windows.KnownFolderPath(windows.FOLDERID_ProgramFiles, 0)
	if err != nil {
		return err
	}
	executable = filepath.Clean(executable)
	allowed := false
	for _, product := range []string{"Driftnet", "Drift"} {
		if strings.EqualFold(executable, filepath.Join(root, product, "drift-router.exe")) {
			allowed = true
			break
		}
	}
	if !allowed {
		return errors.New("Install Driftnet in the protected Program Files location before registering its service")
	}
	// Validate from the trusted parent downward; an unprivileged account cannot
	// exchange a checked child while its parent forbids writes and deletion.
	for _, path := range []string{root, filepath.Dir(executable), executable} {
		if err := validateWindowsFile(path); err != nil {
			return fmt.Errorf("Unsafe networking installation: %w", err)
		}
	}
	return nil
}
