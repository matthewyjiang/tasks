.PHONY: cli-install cli-uninstall cli-run

cli-install:
	cargo install --path cli --force
	@echo "Installed taskmanager to $$HOME/.cargo/bin/taskmanager"
	@echo "If 'taskmanager' is not found, add Cargo's bin dir to PATH: export PATH=\"$$HOME/.cargo/bin:$$PATH\""

cli-uninstall:
	cargo uninstall taskmanager-cli || true

cli-run:
	cargo run -p taskmanager-cli --
