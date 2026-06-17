package test

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLocalDevelopmentToolingFilesExist(t *testing.T) {
	root := ".."
	for _, path := range []string{"Makefile", "docker-compose.yml", "docker-compose.deploy.yml", ".env.example", "README.md", "Dockerfile", ".dockerignore", "scripts/deploy.sh"} {
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

func TestDeployScriptSupportsDockerComposeOperations(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "scripts", "deploy.sh"))
	if err != nil {
		t.Fatalf("read deploy script: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"docker compose", "read -r -p", "docker-compose.deploy.yml", "pull app", "up -d", "/healthz", "HOST_PORT", "PORT=18080", "TASKS_SERVER_IMAGE", "--yes", "status", "logs", "backup", "undeploy", "down --remove-orphans"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("deploy script missing %q", snippet)
		}
	}
	composeData, err := os.ReadFile(filepath.Join("..", "docker-compose.deploy.yml"))
	if err != nil {
		t.Fatalf("read deploy compose file: %v", err)
	}
	composeText := string(composeData)
	if !strings.Contains(composeText, "${TASKS_SERVER_IMAGE:-ghcr.io/matthewyjiang/tasks-server:latest}") {
		t.Fatal("deploy compose file missing configurable GHCR image name")
	}
	if strings.Contains(composeText, "build:") {
		t.Fatal("deploy compose file must not build the app image locally")
	}
	if strings.Contains(text, "--build") {
		t.Fatal("deploy script must not build the app image locally")
	}
	pullIndex := strings.Index(text, "compose pull app")
	upIndex := strings.Index(text, "compose up -d")
	if pullIndex == -1 || upIndex == -1 || pullIndex > upIndex {
		t.Fatal("deploy script must pull the app image before starting services")
	}
	if !strings.Contains(text, "TASKS_SERVER_ENV_FILE=\"$ENV_FILE\" docker compose --env-file \"$ENV_FILE\"") {
		t.Fatal("deploy script must pass custom env files to both Compose interpolation and service env_file")
	}
}

func TestDeployComposeBindsPlaintextPortToLoopback(t *testing.T) {
	data, err := os.ReadFile(filepath.Join("..", "docker-compose.deploy.yml"))
	if err != nil {
		t.Fatalf("read deploy compose: %v", err)
	}
	text := string(data)
	if !strings.Contains(text, "127.0.0.1:${HOST_PORT:-18080}:${PORT:-18080}") {
		t.Fatal("deploy compose must bind the plaintext app port to loopback only")
	}
	if !strings.Contains(text, "${TASKS_SERVER_ENV_FILE:-.env}") {
		t.Fatal("deploy compose must let the deploy script select the service env_file")
	}
	if strings.Contains(text, "- \"${HOST_PORT:-18080}:${PORT:-18080}\"") {
		t.Fatal("deploy compose must not publish the plaintext app port on all host interfaces")
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

func TestServerImagePublishWorkflowExists(t *testing.T) {
	path := filepath.Join("..", "..", ".github", "workflows", "publish-server-image.yml")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read server image publish workflow: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"workflow_dispatch", "tag:", "packages: write", "docker/build-push-action", "context: server", "file: server/Dockerfile", "ref: ${{ steps.tag.outputs.tag }}", "ghcr.io/matthewyjiang/tasks-server:${{ steps.tag.outputs.tag }}", "ghcr.io/matthewyjiang/tasks-server:latest", "server-v"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("server image publish workflow missing %q", snippet)
		}
	}
}

func TestSemanticReleaseDispatchesServerImageWorkflow(t *testing.T) {
	path := filepath.Join("..", "..", "scripts", "semantic_release.py")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read semantic release script: %v", err)
	}
	text := string(data)
	for _, snippet := range []string{"def dispatch_server_image", "publish-server-image.yml", "-f", "tag={tag}", "if args.artifact == \"server\":", "dispatch_server_image(tag)"} {
		if !strings.Contains(text, snippet) {
			t.Fatalf("semantic release script missing %q", snippet)
		}
	}
}
