package main

import (
	"context"
	"encoding/json"
	"errors"
	"golang.org/x/sys/unix"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync/atomic"
	"time"
)

var helperLock *os.File
var removed atomic.Bool

const controlSocket = "/var/run/app.drift.network.sock"

func tunnelName() string { return "" }
func listenControl() (net.Listener, error) {
	if os.Geteuid() != 0 {
		return nil, errors.New("Install the Drift networking helper first")
	}
	var err error
	helperLock, err = os.OpenFile("/var/run/app.drift.network.lock", os.O_CREATE|os.O_RDWR|unix.O_NOFOLLOW, 0600)
	if err != nil {
		return nil, err
	}
	if err = unix.Flock(int(helperLock.Fd()), unix.LOCK_EX|unix.LOCK_NB); err != nil {
		return nil, errors.New("Drift networking helper already runs")
	}

	if st, err := os.Lstat(controlSocket); err == nil {
		if st.Mode()&os.ModeSocket == 0 {
			return nil, errors.New("Invalid helper socket")
		}
		if err = os.Remove(controlSocket); err != nil {
			return nil, err
		}
	}
	l, err := net.Listen("unix", controlSocket)
	if err == nil {
		err = os.Chmod(controlSocket, 0666)
	}
	return l, err
}
func peerOwner(conn net.Conn) (string, error) {
	raw, err := conn.(*net.UnixConn).SyscallConn()
	if err != nil {
		return "", err
	}
	var uid uint32
	err = raw.Control(func(fd uintptr) {
		var c *unix.Xucred
		c, err = unix.GetsockoptXucred(int(fd), unix.SOL_LOCAL, unix.LOCAL_PEERCRED)
		if err == nil {
			uid = c.Uid
		}
	})
	if err != nil {
		return "", err
	}
	if uid == 0 {
		return "", errors.New("A desktop user must start routing")
	}
	// The installer owns this file; the client cannot change the registered user.
	b, err := os.ReadFile("/Library/Application Support/Drift/owner.json")
	if err != nil {
		return "", err
	}
	var record struct {
		UID uint32 `json:"uid"`
	}
	if json.Unmarshal(b, &record) != nil || record.UID != uid {
		return "", errors.New("This user is not registered for Drift")
	}
	return strconv.FormatUint(uint64(uid), 10), nil
}
func proxyCommand(action, url string) error {
	executable, err := os.Executable()
	if err != nil {
		return err
	}
	tool := filepath.Join(filepath.Dir(executable), "app.drift.proxyctl")
	args := []string{action}
	if url != "" {
		args = append(args, url)
	}
	out, err := exec.Command(tool, args...).CombinedOutput()
	if err != nil {
		return errors.New(strings.TrimSpace(string(out)))
	}
	return nil
}
func recoverProxy() error            { return proxyCommand("recover", "") }
func applyProxy(url string) error    { return proxyCommand("apply", url) }
func restoreProxy(url string) error  { return proxyCommand("restore", url) }
func platformCommand() (bool, error) { return false, nil }

func watchApplication(ctx context.Context, cancel context.CancelFunc) {
	ticker := time.NewTicker(5 * time.Second)
	defer ticker.Stop()
	missing := 0
	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			_, currentErr := os.Stat("/Applications/Driftnet.app")
			_, legacyErr := os.Stat("/Applications/Drift.app")
			if os.IsNotExist(currentErr) && os.IsNotExist(legacyErr) {
				missing++
			} else {
				missing = 0
			}
			if missing >= 6 {
				removed.Store(true)
				cancel()
				return
			}
		}
	}
}
func cleanupIfRemoved() error {
	if !removed.Load() {
		return nil
	}
	if err := recoverProxy(); err != nil {
		return err
	}
	for _, path := range []string{"/Library/LaunchDaemons/app.drift.network.plist", "/Library/PrivilegedHelperTools/app.drift.network", "/Library/PrivilegedHelperTools/app.drift.proxyctl", "/Library/PrivilegedHelperTools/app.drift.uninstall", "/var/run/app.drift.network.sock", "/var/run/app.drift.network.lock", "/Library/Application Support/Drift/owner.json", "/Library/Application Support/Drift/proxy-journal.json"} {
		_ = os.Remove(path)
	}
	_ = os.Remove("/Library/Application Support/Drift")
	_ = exec.Command("/usr/sbin/pkgutil", "--forget", "app.drift.desktop.installer").Run()
	_ = exec.Command("/bin/launchctl", "bootout", "system/app.drift.network").Run()
	return nil
}

func dialControl() (net.Conn, error) { return net.DialTimeout("unix", controlSocket, 5*time.Second) }
