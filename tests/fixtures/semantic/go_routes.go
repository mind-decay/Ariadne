// Semantic fixture: Go net/http + Gin HTTP routes
// Expected boundaries:
//   Producers: 4 (HandleFunc /api/health, HandleFunc /api/users, Gin GET /api/items, Gin POST /api/items)
//   Consumers: 0
//   Total: 4

package main

import (
	"net/http"
	"github.com/gin-gonic/gin"
)

func main() {
	// net/http routes
	http.HandleFunc("/api/health", healthHandler)
	http.HandleFunc("/api/users", usersHandler)

	// Gin routes
	r := gin.Default()
	r.GET("/api/items", listItems)
	r.POST("/api/items", createItem)
}
