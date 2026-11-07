// Drift's GPL-3.0-or-later router integration. Upstream attribution: LICENSE.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"os/signal"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"sync"
	"syscall"
	"time"

	box "github.com/sagernet/sing-box"
	"github.com/sagernet/sing-box/adapter/certificate"
	"github.com/sagernet/sing-box/adapter/endpoint"
	"github.com/sagernet/sing-box/adapter/inbound"
	"github.com/sagernet/sing-box/adapter/outbound"
	boxService "github.com/sagernet/sing-box/adapter/service"
	"github.com/sagernet/sing-box/common/process"
	"github.com/sagernet/sing-box/dns"
	"github.com/sagernet/sing-box/dns/transport/local"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing-box/protocol/direct"
	"github.com/sagernet/sing-box/protocol/socks"
	"github.com/sagernet/sing-box/protocol/tun"
	"github.com/sagernet/sing/common/control"
	singJSON "github.com/sagernet/sing/common/json"
)

type App struct {
	Path   string `json:"path"`
	Bundle bool   `json:"bundle"`
}
type Request struct {
	Action         string `json:"action,omitempty"`
	URL            string `json:"url"`
	EnginePort     uint16 `json:"engine_port"`
	InterfaceIndex uint32 `json:"interface_index,omitempty"`
	Apps           []App  `json:"apps"`
}
type Reply struct {
	OK       bool   `json:"ok"`
	Protocol int    `json:"protocol"`
	Router   bool   `json:"router"`
	Error    string `json:"error,omitempty"`
}

func answer(w io.Writer, router bool, err error) {
	r := Reply{OK: err == nil, Protocol: 2, Router: router}
	if err != nil {
		r.Error = err.Error()
	}
	_ = json.NewEncoder(w).Encode(r)
}
func appPatterns(apps []App) ([]string, error) {
	if len(apps) > 128 {
		return nil, errors.New("Too many selected applications")
	}
	patterns := make([]string, 0, len(apps))
	for _, app := range apps {
		if !filepath.IsAbs(app.Path) || strings.ContainsRune(app.Path, 0) {
			return nil, errors.New("Invalid application path")
		}
		path, err := filepath.EvalSymlinks(app.Path)
		if err != nil {
			return nil, fmt.Errorf("Selected application is unavailable: %w", err)
		}
		stat, err := os.Stat(path)
		if err != nil {
			return nil, err
		}
		if app.Bundle {
			if !stat.IsDir() || !strings.HasSuffix(path, ".app") {
				return nil, errors.New("Invalid application bundle")
			}
			path = filepath.Join(path, "Contents") + string(os.PathSeparator)
		} else if !stat.Mode().IsRegular() {
			return nil, errors.New("Select an application executable")
		}
		pattern := "^" + regexp.QuoteMeta(path)
		if app.Bundle {
			pattern += ".*"
		} else {
			pattern += "$"
		}
		if os.PathSeparator == '\\' {
			pattern = "(?i)" + pattern
		}
		patterns = append(patterns, pattern)
	}
	return patterns, nil
}

// Do not make an IPv4-only uplink look IPv6-capable. Advertising a ULA TUN
// address without global upstream IPv6 can strand Electron/Node reconnects.
func ipv6OnInterface(index uint32) bool {
	interfaces, err := net.Interfaces()
	if err != nil {
		return false
	}
	for _, iface := range interfaces {
		if index != 0 && uint32(iface.Index) != index {
			continue
		}
		if iface.Flags&net.FlagUp == 0 || iface.Flags&(net.FlagLoopback|net.FlagPointToPoint) != 0 {
			continue
		}
		addresses, _ := iface.Addrs()
		for _, address := range addresses {
			ip, _, err := net.ParseCIDR(address.String())
			if err == nil && ip.To4() == nil && ip.IsGlobalUnicast() && !ip.IsPrivate() {
				return true
			}
		}
	}
	return false
}
func tunnelAddresses(ipv6 bool) []string {
	addresses := []string{"172.31.253.1/30"}
	if ipv6 {
		addresses = append(addresses, "fd81:4452:4946:5400::1/126")
	}
	return addresses
}
func routerOptions(request Request) ([]byte, error) {
	paths, err := appPatterns(request.Apps)
	if err != nil {
		return nil, err
	}
	if request.EnginePort < 1024 {
		return nil, errors.New("Invalid local engine endpoint")
	}
	// DNS settings and LAN routes are left alone. The default egress binds to the
	// real NIC. Unknown processes and unmatched applications use DIRECT.
	config := map[string]any{
		"log":       map[string]any{"disabled": true},
		"inbounds":  []any{map[string]any{"type": "tun", "tag": "apps", "interface_name": tunnelName(), "address": tunnelAddresses(ipv6OnInterface(request.InterfaceIndex)), "mtu": 1400, "auto_route": true, "dns_mode": "disabled", "strict_route": false, "stack": "gvisor", "route_exclude_address": []string{"127.0.0.0/8", "::1/128", "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "169.254.0.0/16", "fe80::/10", "fc00::/7", "224.0.0.0/4", "ff00::/8"}}},
		"outbounds": []any{map[string]any{"type": "direct", "tag": "direct"}, map[string]any{"type": "socks", "tag": "selected", "server": "127.0.0.1", "server_port": request.EnginePort, "version": "5"}},
		"route":     map[string]any{"auto_detect_interface": true, "final": "direct", "rules": []any{map[string]any{"process_path_regex": paths, "action": "route", "outbound": "selected"}}},
	}
	return json.Marshal(config)
}
func newRouter(ctx context.Context, request Request) (*box.Box, error) {
	ib := inbound.NewRegistry()
	tun.RegisterInbound(ib)
	ob := outbound.NewRegistry()
	direct.RegisterOutbound(ob)
	socks.RegisterOutbound(ob)
	ds := dns.NewTransportRegistry()
	local.RegisterTransport(ds)
	ctx = box.Context(ctx, ib, ob, endpoint.NewRegistry(), ds, boxService.NewRegistry(), certificate.NewRegistry())
	raw, err := routerOptions(request)
	if err != nil {
		return nil, err
	}
	options, err := singJSON.UnmarshalExtendedContext[option.Options](ctx, raw)
	if err != nil {
		return nil, err
	}
	return box.New(box.Options{Context: ctx, Options: options})
}

var sessionLock sync.Mutex

func serve(ctx context.Context) error {
	if err := recoverProxy(); err != nil {
		return err
	}
	listener, err := listenControl()
	if err != nil {
		return err
	}
	defer listener.Close()
	slots := make(chan struct{}, 32)
	var workers sync.WaitGroup
	defer workers.Wait()
	go func() { <-ctx.Done(); _ = listener.Close() }()
	for {
		conn, err := listener.Accept()
		if err != nil {
			if ctx.Err() != nil {
				return nil
			}
			return err
		}
		select {
		case slots <- struct{}{}:
		default:
			conn.Close()
			continue
		}
		workers.Add(1)
		go func() { defer workers.Done(); defer func() { <-slots }(); handle(ctx, conn) }()
	}
}
func handle(ctx context.Context, conn net.Conn) {
	defer conn.Close()
	_ = conn.SetReadDeadline(time.Now().Add(8 * time.Second))
	reader := bufio.NewReader(io.LimitReader(conn, 65537))
	line, err := reader.ReadBytes('\n')
	if err != nil || len(line) > 65536 {
		answer(conn, false, errors.New("Invalid network request"))
		return
	}
	// Windows impersonation requires a message to have been read first.
	// The bounded read above performs no privileged action.
	owner, err := peerOwner(conn)
	if err != nil {
		answer(conn, false, err)
		return
	}
	var request Request
	if err = json.Unmarshal(line, &request); err != nil {
		answer(conn, false, err)
		return
	}
	if request.Action == "stats" {
		_ = json.NewEncoder(conn).Encode(process.DriftStats(owner))
		return
	}
	if !sessionLock.TryLock() {
		answer(conn, false, errors.New("Another Drift session is active"))
		return
	}
	defer sessionLock.Unlock()
	if err = recoverProxy(); err != nil {
		answer(conn, false, err)
		return
	}
	if request.URL == "" && len(request.Apps) == 0 {
		answer(conn, false, errors.New("No routing rules selected"))
		return
	}
	_ = conn.SetReadDeadline(time.Time{})
	sessionCtx, cancel := context.WithCancel(ctx)
	defer cancel()
	go func() { var stop [1]byte; _, _ = conn.Read(stop[:]); cancel() }()
	var router *box.Box
	if len(request.Apps) > 0 {
		patterns, patternErr := appPatterns(request.Apps)
		if patternErr != nil {
			answer(conn, false, patternErr)
			return
		}
		if err = process.SetDriftScope(owner, patterns); err != nil {
			answer(conn, false, err)
			return
		}
		router, err = newRouter(sessionCtx, request)
		if err == nil {
			err = router.Start()
		}
		if router != nil {
			defer func() {
				if router != nil {
					_ = router.Close()
				}
			}()
		}
		if err != nil {
			answer(conn, false, fmt.Errorf("Application routing could not start: %w", err))
			return
		}
	}
	if router != nil {
		monitor := router.Network().InterfaceMonitor()
		if monitor != nil {
			original := monitor.DefaultInterface()
			if original != nil {
				index := original.Index
				callback := monitor.RegisterCallback(func(current *control.Interface, _ int) {
					if current == nil || current.Index != index {
						cancel()
					}
				})
				defer monitor.UnregisterCallback(callback)
			}
		}
	}
	if sessionCtx.Err() != nil {
		return
	}
	if request.URL != "" {
		if err = applyProxy(request.URL); err != nil {
			answer(conn, false, err)
			return
		}
		defer restoreProxy(request.URL)
	}
	answer(conn, router != nil, nil)
	<-sessionCtx.Done()
	// Explicit teardown before acknowledging stop. No listener survives the lease.
	if request.URL != "" {
		_ = restoreProxy(request.URL)
		request.URL = ""
	}
	if router != nil {
		_ = router.Close()
		router = nil
	}
	answer(conn, false, nil)
}
func run() error {
	if len(os.Args) > 1 && os.Args[1] == "--stats" {
		c, err := dialControl()
		if err != nil {
			return err
		}
		defer c.Close()
		_ = c.SetDeadline(time.Now().Add(5 * time.Second))
		if err = json.NewEncoder(c).Encode(Request{Action: "stats"}); err != nil {
			return err
		}
		_, err = io.Copy(os.Stdout, c)
		return err
	}

	if len(os.Args) > 1 && os.Args[1] == "--interface-index" {
		socket, err := net.Dial("udp4", "1.1.1.1:53")
		if err != nil {
			return err
		}
		ip := socket.LocalAddr().(*net.UDPAddr).IP
		socket.Close()
		interfaces, err := net.Interfaces()
		if err != nil {
			return err
		}
		for _, iface := range interfaces {
			addresses, _ := iface.Addrs()
			for _, address := range addresses {
				candidate, _, _ := net.ParseCIDR(address.String())
				if candidate.Equal(ip) {
					if iface.Flags&net.FlagPointToPoint != 0 {
						return errors.New("Another network tunnel is active")
					}
					fmt.Println(strconv.Itoa(iface.Index))
					return nil
				}
			}
		}
		return errors.New("Default network interface unavailable")
	}
	if len(os.Args) > 1 && os.Args[1] == "--check" {
		var r Request
		if err := json.NewDecoder(io.LimitReader(os.Stdin, 65536)).Decode(&r); err != nil {
			return err
		}
		b, err := newRouter(context.Background(), r)
		if err == nil {
			_ = b.Close()
		}
		return err
	}
	if handled, err := platformCommand(); handled {
		return err
	}
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()
	go watchApplication(ctx, cancel)
	err := serve(ctx)
	if cleanupErr := cleanupIfRemoved(); err == nil {
		err = cleanupErr
	}
	return err
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
