//go:build !windows

package main

import "net"

func peerAppPatterns(_ net.Conn, _ string, apps []App) ([]string, error) {
	return appPatterns(apps)
}
