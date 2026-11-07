package main

import (
	"net"
	"os"
	"path/filepath"
	"strconv"
	"testing"
)

func TestLinuxPeerOwnerComesFromKernel(t *testing.T) {
	path := filepath.Join(t.TempDir(), "peer.sock")
	listener, err := net.Listen("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	client, err := net.Dial("unix", path)
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close()
	server, err := listener.Accept()
	if err != nil {
		t.Fatal(err)
	}
	defer server.Close()
	owner, err := peerOwner(server)
	if os.Geteuid() == 0 {
		if err == nil {
			t.Fatal("root callers must not own desktop routing")
		}
	} else if err != nil || owner != strconv.Itoa(os.Geteuid()) {
		t.Fatalf("owner=%q error=%v, want kernel uid %d", owner, err, os.Geteuid())
	}
}
func TestLinuxPeerOwnerRejectsNonUnix(t *testing.T) {
	left, right := net.Pipe()
	defer left.Close()
	defer right.Close()
	if _, err := peerOwner(left); err == nil {
		t.Fatal("accepted unauthenticated connection")
	}
}
func TestLinuxHelperDoesNotSetUserProxyAsRoot(t *testing.T) {
	if err := applyProxy("http://127.0.0.1:42000/proxy.pac"); err == nil {
		t.Fatal("root helper accepted per-user proxy configuration")
	}
}
