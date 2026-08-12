// Package core is a tiny shared Go library used by the worker app.
package core

// Message returns a greeting from the shared core package.
func Message() string {
	return "hello from acme-corp's shared Go package"
}