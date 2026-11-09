package main

import (
	"context"
	"errors"
	"fmt"
	"github.com/Microsoft/go-winio"
	"github.com/sagernet/sing-box/common/process"
	"golang.org/x/sys/windows"
	"golang.org/x/sys/windows/svc"
	"golang.org/x/sys/windows/svc/mgr"
	"net"
	"os"
	"path/filepath"
	"syscall"
	"time"
)

const serviceName = "DriftNetwork"

func tunnelName() string { return "Drift" }
func listenControl() (net.Listener, error) {
	return winio.ListenPipe(`\\.\pipe\Drift.Network`, &winio.PipeConfig{SecurityDescriptor: "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GRGW;;;IU)", InputBufferSize: 65536, OutputBufferSize: 4096})
}
func peerOwner(conn net.Conn) (string, error) {
	f, ok := conn.(interface{ Fd() uintptr })
	if !ok {
		return "", errors.New("Cannot authenticate this control pipe")
	}
	var owner string
	err := process.DriftWithImpersonation(func() error {
		adv := windows.NewLazySystemDLL("advapi32.dll")
		okCode, _, err := adv.NewProc("ImpersonateNamedPipeClient").Call(f.Fd())
		if okCode == 0 {
			return err
		}
		return nil
	}, func() error {
		var token windows.Token
		if err := windows.OpenThreadToken(windows.CurrentThread(), windows.TOKEN_QUERY, true, &token); err != nil {
			return err
		}
		defer token.Close()
		user, err := token.GetTokenUser()
		if err != nil {
			return err
		}
		owner = user.User.Sid.String()
		return nil
	})
	return owner, err
}

func peerAppPatterns(conn net.Conn, owner string, apps []App) ([]string, error) {
	pipe, ok := conn.(interface{ Fd() uintptr })
	if !ok {
		return nil, errors.New("Cannot authenticate this control pipe")
	}
	var pid uint32
	if err := windows.GetNamedPipeClientProcessId(windows.Handle(pipe.Fd()), &pid); err != nil {
		return nil, err
	}
	client, err := windows.OpenProcess(windows.PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
	if err != nil {
		return nil, err
	}
	defer windows.CloseHandle(client)
	var token windows.Token
	if err = windows.OpenProcessToken(client, windows.TOKEN_QUERY|windows.TOKEN_DUPLICATE, &token); err != nil {
		return nil, err
	}
	defer token.Close()
	user, err := token.GetTokenUser()
	if err != nil {
		return nil, err
	}
	if owner == "" || user.User.Sid.String() != owner {
		return nil, windows.ERROR_ACCESS_DENIED
	}
	var patterns []string
	err = process.DriftWithProcessUser(token, func() (err error) {
		patterns, err = appPatterns(apps)
		return err
	})
	return patterns, err
}

func recoverProxy() error { return nil }
func applyProxy(_ string) error {
	return errors.New("Windows website proxy settings must be applied in the desktop user's session")
}
func restoreProxy(_ string) error { return nil }

type serviceHandler struct{}

func (serviceHandler) Execute(_ []string, requests <-chan svc.ChangeRequest, status chan<- svc.Status) (bool, uint32) {
	status <- svc.Status{State: svc.StartPending}
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()
	done := make(chan error, 1)
	go func() { done <- serve(ctx) }()
	status <- svc.Status{State: svc.Running, Accepts: svc.AcceptStop | svc.AcceptShutdown}
	for {
		select {
		case err := <-done:
			if err != nil {
				return false, 1
			}
			return false, 0
		case r := <-requests:
			switch r.Cmd {
			case svc.Interrogate:
				status <- r.CurrentStatus
			case svc.Stop, svc.Shutdown:
				status <- svc.Status{State: svc.StopPending}
				cancel()
				<-done
				return false, 0
			}
		}
	}
}
func platformCommand() (bool, error) {
	if len(os.Args) < 2 {
		return false, nil
	}
	mode := os.Args[1]
	if mode == "--service" {
		exe, err := os.Executable()
		if err != nil {
			return true, err
		}
		if err := validateWindowsServiceExecutable(exe); err != nil {
			return true, err
		}
		return true, svc.Run(serviceName, serviceHandler{})
	}
	if mode != "--install-service" && mode != "--uninstall-service" {
		return false, nil
	}
	manager, err := mgr.Connect()
	if err != nil {
		return true, fmt.Errorf("open service manager: %w", err)
	}
	defer manager.Disconnect()
	service, err := manager.OpenService(serviceName)
	if mode == "--uninstall-service" {
		if err != nil {
			return true, nil
		}
		defer service.Close()
		_, _ = service.Control(svc.Stop)
		until := time.Now().Add(20 * time.Second)
		for {
			state, queryErr := service.Query()
			if queryErr != nil || state.State == svc.Stopped {
				break
			}
			if time.Now().After(until) {
				return true, errors.New("Drift service is still stopping")
			}
			time.Sleep(100 * time.Millisecond)
		}
		return true, service.Delete()
	}
	exe, err := os.Executable()
	if err != nil {
		return true, fmt.Errorf("resolve service executable: %w", err)
	}
	exe, err = filepath.Abs(exe)
	if err != nil {
		return true, err
	}
	if err := validateWindowsServiceExecutable(exe); err != nil {
		return true, err
	}
	config := mgr.Config{DisplayName: "Driftnet Networking", Description: "Routes only applications selected in Driftnet; other connections are direct.", ServiceType: windows.SERVICE_WIN32_OWN_PROCESS, StartType: mgr.StartAutomatic, ErrorControl: mgr.ErrorNormal}
	if service == nil {
		service, err = manager.CreateService(serviceName, exe, config, "--service")
	} else {
		config.BinaryPathName = syscall.EscapeArg(exe) + " --service"
		err = service.UpdateConfig(config)
	}
	if err != nil {
		return true, fmt.Errorf("create or update networking service: %w", err)
	}
	defer service.Close()
	if err = service.SetRecoveryActions([]mgr.RecoveryAction{{Type: mgr.ServiceRestart, Delay: 3 * time.Second}}, 86400); err != nil {
		return true, fmt.Errorf("configure service recovery: %w", err)
	}
	_ = service.SetRecoveryActionsOnNonCrashFailures(true)
	err = service.Start()
	if err != nil && err != windows.ERROR_SERVICE_ALREADY_RUNNING {
		return true, fmt.Errorf("start networking service: %w", err)
	}
	if err := waitWindowsPipe(10*time.Second, func(timeout *time.Duration) (net.Conn, error) {
		return winio.DialPipe(`\\.\pipe\Drift.Network`, timeout)
	}); err != nil {
		return true, fmt.Errorf("connect networking service pipe: %w", err)
	}
	return true, nil
}

func waitWindowsPipe(timeout time.Duration, dial func(*time.Duration) (net.Conn, error)) error {
	deadline := time.Now().Add(timeout)
	for {
		remaining := time.Until(deadline)
		if remaining <= 0 {
			return errors.New("Networking service control pipe did not become ready")
		}
		pipe, err := dial(&remaining)
		if err == nil {
			return pipe.Close()
		}
		// StartService can return after SERVICE_RUNNING but before the server's
		// first pipe instance exists. go-winio does not retry FILE_NOT_FOUND.
		if !errors.Is(err, windows.ERROR_FILE_NOT_FOUND) && !errors.Is(err, windows.ERROR_PIPE_BUSY) && !errors.Is(err, winio.ErrTimeout) {
			return err
		}
		time.Sleep(25 * time.Millisecond)
	}
}

func watchApplication(_ context.Context, _ context.CancelFunc) {}
func cleanupIfRemoved() error                                  { return nil }

func dialControl() (net.Conn, error) {
	t := 5 * time.Second
	return winio.DialPipe(`\\.\pipe\Drift.Network`, &t)
}
