.PHONY: cli-install cli-uninstall cli-run

cli-install:
	cargo install --path cli --force
	@echo "Installed tsk to $$HOME/.cargo/bin/tsk"
	@echo "If 'tsk' is not found, add Cargo's bin dir to PATH: export PATH=\"$$HOME/.cargo/bin:$$PATH\""

cli-uninstall:
	cargo uninstall taskmanager-cli || true

cli-run:
	cargo run -p taskmanager-cli --
