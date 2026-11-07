package process

import (
	"github.com/sagernet/sing-box/adapter"
	"regexp"
	"sync/atomic"
)

type driftScopeState struct {
	owner    string
	patterns []*regexp.Regexp
	selected atomic.Uint64
	direct   atomic.Uint64
	unknown  atomic.Uint64
}

var driftScope atomic.Pointer[driftScopeState]

// Only the authenticated helper calls this. No environment variable controls scope.
func SetDriftScope(owner string, patterns []string) error {
	state := &driftScopeState{owner: owner}
	for _, pattern := range patterns {
		compiled, err := regexp.Compile(pattern)
		if err != nil {
			return err
		}
		state.patterns = append(state.patterns, compiled)
	}
	driftScope.Store(state)
	return nil
}
func driftExpectedOwner() string {
	s := driftScope.Load()
	if s == nil {
		return ""
	}
	return s.owner
}
func driftRecord(info *adapter.ConnectionOwner) {
	s := driftScope.Load()
	if s == nil {
		return
	}
	for _, pattern := range s.patterns {
		if pattern.MatchString(info.ProcessPath) {
			s.selected.Add(1)
			return
		}
	}
	s.direct.Add(1)
}
func DriftStats(owner string) map[string]uint64 {
	s := driftScope.Load()
	if s == nil || s.owner != owner {
		return map[string]uint64{}
	}
	return map[string]uint64{"selected": s.selected.Load(), "direct": s.direct.Load(), "unknown": s.unknown.Load()}
}
func driftUnknown() {
	if s := driftScope.Load(); s != nil {
		s.unknown.Add(1)
	}
}
