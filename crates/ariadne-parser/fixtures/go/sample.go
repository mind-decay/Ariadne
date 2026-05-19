package sample

import (
	"fmt"
	"strings"
)

type Counter struct {
	Value int
}

type Direction int

func (c *Counter) Increment() int {
	c.Value++
	return c.Value
}

func New(start int) *Counter {
	return &Counter{Value: start}
}

func describe(c *Counter) string {
	return fmt.Sprintf("counter=%d", c.Value)
}

func joinParts(parts []string) string {
	return strings.Join(parts, "/")
}
