package test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLocalDevelopmentToolingFilesExist(t *testing.T) {
	root := ".."
	for _, path := range []string{"Makefile", "docker-compose.yml", ".env.example", "README.md"} {
		if _, err := os.Stat(filepath.Join(root, path)); err != nil {
			t.Fatalf("expected %s to exist: %v", path, err)
		}
	}
}

func TestMakefileExposesCheckTarget(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "Makefile"))
	if err != nil {
		t.Fatalf("read Makefile: %v", err)
	}
	text := string(data)
	for _, target := range []string{"test:", "build:", "check:"} {
		if !strings.Contains(text, target) {
			t.Fatalf("Makefile missing target %s", target)
		}
	}
}

func TestCIWorkflowExists(t *testing.T) {
	path := filepath.Join("..", "..", ".github", "workflows", "server.yml")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read CI workflow: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"go test ./...", "go build ./...", "working-directory: server"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("CI workflow missing %q", snippet)
		}
	}
}
