package main

import (
	"fmt"
	"log"
	"net/http"
	"os"

	"acme-corp/packages/core"
)

func main() {
	addr := os.Getenv("ADDR")
	if addr == "" {
		addr = ":9000"
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, _ *http.Request) {
		fmt.Fprintf(w, "<h1>worker</h1><p>%s</p>\n", core.Message())
	})
	log.Printf("[worker] listening on %s (NODE_ENV=%s)", addr, os.Getenv("NODE_ENV"))
	log.Fatal(http.ListenAndServe(addr, mux))
}