// Package sample exercises Go branch-sensitive extraction.
package sample

// Server holds connection state.
type Server struct {
	Addr string
}

// Start begins serving and returns the bind address.
func (s *Server) Start() string {
	endpoint := "127.0.0.1:8080"
	return endpoint
}

// LegacyStart is the old entry point.
//
// Deprecated: use Start instead.
func (s *Server) LegacyStart() string {
	return "legacy"
}

// PublicHelper is an exported free function.
func PublicHelper() int {
	return 1
}

// privateHelper is unexported.
func privateHelper() int {
	return 0
}

// TestServerStart is a Go test entry point.
func TestServerStart() {
	_ = (&Server{}).Start()
}
