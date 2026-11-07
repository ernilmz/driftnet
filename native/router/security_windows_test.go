package main

import (
	"net"
	"testing"
	"time"

	"golang.org/x/sys/windows"
)

func TestServiceSecurityDescriptorRejectsWritableOrUnownedCode(t *testing.T) {
	for _, test := range []struct {
		sddl string
		ok   bool
	}{
		{"O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRFX;;;BU)", true},
		{"O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FA;;;BU)", false},
		{"O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;WD;;;WD)", false},
		{"O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;WO;;;AU)", false},
		{"O:BUG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)(A;;FRFX;;;BU)", false},
		{"O:BAG:BAD:NO_ACCESS_CONTROL", false},
	} {
		descriptor, err := windows.SecurityDescriptorFromString(test.sddl)
		if err != nil {
			t.Fatal(err)
		}
		if got := validateWindowsDescriptor(descriptor) == nil; got != test.ok {
			t.Fatalf("descriptor accepted=%v, expected %v", got, test.ok)
		}
	}
}

func TestServiceReadinessWaitsForFirstPipeInstance(t *testing.T) {
	attempts := 0
	err := waitWindowsPipe(time.Second, func(_ *time.Duration) (net.Conn, error) {
		attempts++
		if attempts < 3 {
			return nil, windows.ERROR_FILE_NOT_FOUND
		}
		client, server := net.Pipe()
		defer server.Close()
		return client, nil
	})
	if err != nil || attempts != 3 {
		t.Fatalf("readiness attempts=%d, error=%v", attempts, err)
	}
}

func TestServiceCannotBeRegisteredFromUserWritableFolder(t *testing.T) {
	if err := validateWindowsServiceExecutable(t.TempDir() + `\drift-router.exe`); err == nil {
		t.Fatal("service executable outside protected Program Files was accepted")
	}
}
