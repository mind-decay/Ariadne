package main

import (
	"fmt"

	"github.com/example/myproject/internal/auth"
)

func main() {
	result := auth.Authenticate("admin", "secret")
	fmt.Println("Auth result:", result)
}
