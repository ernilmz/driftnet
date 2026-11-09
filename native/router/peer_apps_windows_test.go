package main

import (
	"errors"
	"fmt"
	"github.com/Microsoft/go-winio"
	"golang.org/x/sys/windows"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"testing"
	"time"
)

func TestWindowsIdentificationPipeValidatesAppsAsAuthenticatedProcess(t *testing.T) {
	token := windows.GetCurrentProcessToken()
	user, err := token.GetTokenUser()
	if err != nil {
		t.Fatal(err)
	}
	sid := user.User.Sid.String()
	name := fmt.Sprintf(`\\.\pipe\Driftnet.Peer.Test.%d`, os.Getpid())
	listener, err := winio.ListenPipe(name, &winio.PipeConfig{SecurityDescriptor: "D:P(A;;GA;;;" + sid + ")", InputBufferSize: 4096, OutputBufferSize: 4096})
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	app := filepath.Join(t.TempDir(), "Selected.exe")
	if err = os.WriteFile(app, []byte("fixture"), 0600); err != nil {
		t.Fatal(err)
	}
	done := make(chan error, 1)
	go func() {
		runtime.LockOSThread()
		defer runtime.UnlockOSThread()
		conn, err := listener.Accept()
		if err != nil {
			done <- err
			return
		}
		defer conn.Close()
		var message [1]byte
		if _, err = io.ReadFull(conn, message[:]); err != nil {
			done <- err
			return
		}
		owner, err := peerOwner(conn)
		if err != nil {
			done <- err
			return
		}
		if owner != sid {
			done <- errors.New("pipe owner SID mismatch")
			return
		}
		patterns, err := peerAppPatterns(conn, owner, []App{{Path: app}})
		if err != nil {
			done <- err
			return
		}
		metadata, err := filepath.EvalSymlinks(app)
		if err != nil {
			done <- err
			return
		}
		if len(patterns) != 1 || !regexp.MustCompile(patterns[0]).MatchString(normalizeAppPath(metadata)) {
			done <- errors.New("identified pipe client could not validate its app")
			return
		}
		// This must fail on identity before attempting any remote path access.
		_, err = peerAppPatterns(conn, "S-1-0-0", []App{{Path: `\\127.0.0.1\never-access-this-share\app.exe`}})
		if !errors.Is(err, windows.ERROR_ACCESS_DENIED) {
			done <- fmt.Errorf("foreign peer was not denied before path access: %v", err)
			return
		}
		var after windows.Token
		err = windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &after)
		if err != windows.ERROR_NO_TOKEN {
			if err == nil {
				after.Close()
			}
			done <- fmt.Errorf("pipe validation left thread impersonating: %v", err)
			return
		}
		done <- nil
	}()
	wide, err := windows.UTF16PtrFromString(name)
	if err != nil {
		t.Fatal(err)
	}
	handle, err := windows.CreateFile(wide, windows.GENERIC_READ|windows.GENERIC_WRITE, 0, nil, windows.OPEN_EXISTING, windows.SECURITY_SQOS_PRESENT|windows.SECURITY_IDENTIFICATION, 0)
	if err != nil {
		t.Fatal(err)
	}
	// SAFETY: unique valid handle returned by CreateFile, transferred to File.
	client := os.NewFile(uintptr(handle), name)
	defer client.Close()
	if _, err = client.Write([]byte{1}); err != nil {
		t.Fatal(err)
	}
	select {
	case err := <-done:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(10 * time.Second):
		t.Fatal("authenticated pipe validation timed out")
	}
}
