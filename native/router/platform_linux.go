package main

import (
	"context"
	"errors"
	"golang.org/x/sys/unix"
	"net"
	"os"
	"strconv"
	"syscall"
	"time"
)

const controlSocket = "/run/driftnet/network.sock"

var helperLock *os.File

func tunnelName() string { return "driftnet0" }
func listenControl() (net.Listener, error) {
	if os.Geteuid() != 0 {
		return nil, errors.New("Install the Driftnet networking service first")
	}
	// systemd creates this root-owned directory. Never follow a replaced directory.
	st, err := os.Lstat("/run/driftnet")
	if err != nil {
		return nil, err
	}
	owner, ok := st.Sys().(*syscall.Stat_t)
	if !st.IsDir() || !ok || owner.Uid != 0 || st.Mode().Perm()&0022 != 0 {
		return nil, errors.New("Unsafe networking runtime directory")
	}
	helperLock, err = os.OpenFile("/run/driftnet/network.lock", os.O_CREATE|os.O_RDWR|unix.O_NOFOLLOW, 0600)
	if err != nil {
		return nil, err
	}
	if err = unix.Flock(int(helperLock.Fd()), unix.LOCK_EX|unix.LOCK_NB); err != nil {
		return nil, errors.New("Driftnet networking service already runs")
	}
	if st, err := os.Lstat(controlSocket); err == nil {
		owner, ok := st.Sys().(*syscall.Stat_t)
		if st.Mode()&os.ModeSocket == 0 || !ok || owner.Uid != 0 {
			return nil, errors.New("Unsafe networking socket")
		}
		if err := os.Remove(controlSocket); err != nil {
			return nil, err
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	listener, err := net.Listen("unix", controlSocket)
	if err != nil {
		return nil, err
	}
	if err := os.Chmod(controlSocket, 0666); err != nil {
		listener.Close()
		return nil, err
	}
	return listener, nil
}
func peerOwner(conn net.Conn) (string, error) {
	socket, ok := conn.(*net.UnixConn)
	if !ok {
		return "", errors.New("Expected a local Unix connection")
	}
	raw, err := socket.SyscallConn()
	if err != nil {
		return "", err
	}
	var credentials *unix.Ucred
	var credentialErr error
	if err := raw.Control(func(fd uintptr) {
		credentials, credentialErr = unix.GetsockoptUcred(int(fd), unix.SOL_SOCKET, unix.SO_PEERCRED)
	}); err != nil {
		return "", err
	}
	if credentialErr != nil {
		return "", credentialErr
	}
	if credentials == nil || credentials.Uid == 0 {
		return "", errors.New("A desktop user must start routing")
	}
	// Socket credentials are supplied by the kernel; the client cannot claim another UID.
	return strconv.FormatUint(uint64(credentials.Uid), 10), nil
}
func recoverProxy() error { return nil }
func applyProxy(string) error {
	return errors.New("Website routing must be configured in the desktop user's GNOME session")
}
func restoreProxy(string) error                                  { return nil }
func platformCommand() (bool, error)                             { return false, nil }
func watchApplication(ctx context.Context, _ context.CancelFunc) { <-ctx.Done() }
func cleanupIfRemoved() error                                    { return nil }
func dialControl() (net.Conn, error)                             { return net.DialTimeout("unix", controlSocket, 5*time.Second) }
