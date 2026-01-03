// Gin example with the same interface:
// Route: /ai/v1/v1/api/square (GET)
// Response: {"code":200,"msg":"ok","data":{"result":"echo: <keyword>"}}
// Run: go run gin_server.go
// Stop: Ctrl+C

package main

import (
	"net/http"

	"github.com/gin-gonic/gin"
)

type AppContext struct{}

type SquareListReq struct {
	Keyword string `form:"keyword"`
}

type SquareListResp struct {
	Code int         `json:"code"`
	Msg  string      `json:"msg"`
	Data interface{} `json:"data"`
}

func squareListHandler(app AppContext) gin.HandlerFunc {
	return func(c *gin.Context) {
		var req SquareListReq
		_ = c.ShouldBindQuery(&req) // keyword defaults to empty
		resp := SquareListResp{
			Code: 200,
			Msg:  "ok",
			Data: map[string]string{"result": "echo: " + req.Keyword},
		}
		c.JSON(http.StatusOK, resp)
	}
}

// Middleware: add response header.
func addHeaderMiddleware() gin.HandlerFunc {
	return func(c *gin.Context) {
		c.Next()
		c.Writer.Header().Set("X-Example", "1")
	}
}

func main() {
	app := AppContext{}
	r := gin.New()
	r.Use(gin.Recovery())
	// Same order as halo-rest example: only add header.
	r.Use(addHeaderMiddleware())

	r.GET("/ai/v1/v1/api/square", squareListHandler(app))

	addr := ":8082"
	println("Gin listening on http://127.0.0.1" + addr + "/ai/v1/v1/api/square/")
	println("Press Ctrl+C to stop.")
	if err := r.Run(addr); err != nil {
		panic(err)
	}
}

