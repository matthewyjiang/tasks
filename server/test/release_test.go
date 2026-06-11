package test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReleaseWorkflowUsesPathScopedArtifacts(t *testing.T) {
	path := filepath.Join("..", "..", ".github", "workflows", "release.yml")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read release workflow: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"branches:", "- main", "scripts/semantic_release.py", "artifact: server", "artifact: core", "artifact: linux-app", "fetch-depth: 0"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("release workflow missing %q", snippet)
		}
	}
}

func TestSemanticReleaseScriptDocumentsArtifactTags(t *testing.T) {
	path := filepath.Join("..", "..", "scripts", "semantic_release.py")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read semantic release script: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"linux-app-v1.2.3", "--artifact", "--path", "BREAKING CHANGE", "git tag"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("semantic release script missing %q", snippet)
		}
	}
}

func TestReleaseDocsDescribePerArtifactTags(t *testing.T) {
	path := filepath.Join("..", "..", "RELEASE.md")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read RELEASE.md: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"server-vX.Y.Z", "core-vX.Y.Z", "linux-app-vX.Y.Z", "path-scoped"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("release docs missing %q", snippet)
		}
	}
}
