package main

import (
	"fmt"
	"net/http"
)

func main() {
	http.HandleFunc("/api/health", func(w http.ResponseWriter, r *http.Request) {
		fmt.Fprintln(w, `{"status": "ok"}`)
	})
	fmt.Println("Backend listening on :8080")
	http.ListenAndServe(":8080", nil)
}
