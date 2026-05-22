// Package fixture is a minimal, dependency-free Go module indexed by the
// scip-go driver test in tests/ingest_go.rs.
package fixture

// Demo carries a single field so the indexer emits a struct symbol and a
// field symbol.
type Demo struct {
	Field int
}

// Run reads the field, giving the indexer a function definition plus a
// reference occurrence to resolve.
func Run(d Demo) int {
	return d.Field
}
