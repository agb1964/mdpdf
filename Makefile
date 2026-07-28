# Частые команды mdpdf. `make` без аргументов печатает справку.

CARGO ?= cargo
BIN   := target/release/mdpdf
SAMPLE ?= /tmp/mdpdf-sample.md
RELEASE_REMOTE ?= origin
RELEASE_BRANCH ?= master

.DEFAULT_GOAL := help
.PHONY: help ci fmt fmt-check check clippy test doc deny build release \
        release-tag run size coverage fuzz tools clean sample golden-update

help: ## Показать список целей
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

## --- Обязательные проверки (ТЗ §21, §22, §28) --------------------------------

ci: fmt-check check clippy test doc deny ## Обязательные локальные проверки

fmt: ## Отформатировать код
	$(CARGO) fmt --all

fmt-check: ## Проверить форматирование
	$(CARGO) fmt --all --check

check: ## cargo check
	$(CARGO) check --all-targets --all-features

clippy: ## Линтер, предупреждения = ошибки
	$(CARGO) clippy --all-targets --all-features -- -D warnings

test: ## Тесты
	$(CARGO) test --all-targets --all-features

doc: ## Документация без зависимостей
	$(CARGO) doc --no-deps

deny: ## Лицензии, advisories, источники зависимостей
	$(CARGO) deny check

## --- Сборка и запуск ---------------------------------------------------------

build: ## Debug-сборка
	$(CARGO) build

release: ## Release-сборка
	$(CARGO) build --release

release-tag: ## Создать тег v<version из Cargo.toml> и отправить в remote
	@set -eu; \
	package_id="$$( $(CARGO) pkgid --locked --manifest-path Cargo.toml )"; \
	version="$${package_id##*@}"; \
	tag="v$$version"; \
	branch="$$(git branch --show-current)"; \
	if [ "$$branch" != "$(RELEASE_BRANCH)" ]; then \
		echo "release-tag: нужна ветка $(RELEASE_BRANCH), сейчас '$${branch:-detached HEAD}'." >&2; \
		exit 1; \
	fi; \
	if ! git diff --quiet || ! git diff --cached --quiet; then \
		echo "release-tag: рабочее дерево не чистое; сначала закоммитьте версию и документацию." >&2; \
		exit 1; \
	fi; \
	git fetch --quiet "$(RELEASE_REMOTE)" "$(RELEASE_BRANCH)"; \
	local_head="$$(git rev-parse HEAD)"; \
	remote_head="$$(git rev-parse "$(RELEASE_REMOTE)/$(RELEASE_BRANCH)")"; \
	if [ "$$local_head" != "$$remote_head" ]; then \
		echo "release-tag: HEAD должен совпадать с $(RELEASE_REMOTE)/$(RELEASE_BRANCH)." >&2; \
		exit 1; \
	fi; \
	if git ls-remote --exit-code --tags "$(RELEASE_REMOTE)" "refs/tags/$$tag" >/dev/null 2>&1; then \
		echo "release-tag: тег $$tag уже опубликован в $(RELEASE_REMOTE)." >&2; \
		exit 1; \
	fi; \
	if git rev-parse --verify --quiet "refs/tags/$$tag" >/dev/null; then \
		tag_type="$$(git cat-file -t "refs/tags/$$tag")"; \
		tag_head="$$(git rev-list -n 1 "$$tag")"; \
		if [ "$$tag_type" != "tag" ] || [ "$$tag_head" != "$$local_head" ]; then \
			echo "release-tag: локальный тег $$tag существует, но это не аннотированный тег текущего HEAD." >&2; \
			exit 1; \
		fi; \
		echo "release-tag: повторная отправка локального тега $$tag."; \
	else \
		echo "release-tag: создаю аннотированный тег $$tag."; \
		git tag -a "$$tag" -m "Release $$tag"; \
	fi; \
	git push "$(RELEASE_REMOTE)" "refs/tags/$$tag:refs/tags/$$tag"; \
	echo "release-tag: $$tag опубликован в $(RELEASE_REMOTE)."

run: build ## Запустить на тестовом документе (make run SAMPLE=path.md)
	$(CARGO) run -- --verbose $(SAMPLE)

sample: ## Создать тестовый Markdown (путь в переменной SAMPLE)
	@printf '# Заголовок\n\nТекст с **жирным** и *курсивом*.\n\n- пункт\n- пункт\n' > $(SAMPLE)
	@echo "written $(SAMPLE)"

size: release ## Размер release-бинарника (ТЗ §18)
	@ls -l $(BIN) | awk '{printf "release binary: %s bytes (%.1f MiB)\n", $$5, $$5/1048576}'

## --- Дополнительные проверки -------------------------------------------------

golden-update: ## Перезаписать эталонные AST-, Typst- и PDF-файлы (ТЗ §19)
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test markdown_parser
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test typst_generator
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test golden_pdf pdf_structure_matches_golden_facts

coverage: ## Информационный отчёт о покрытии тестами (ТЗ §19.3)
	$(CARGO) llvm-cov --all-features --workspace --summary-only

FUZZ_TIME ?= 60

fuzz: ## Fuzzing, по $(FUZZ_TIME)с на таргет (ТЗ §19.4, требует nightly и cargo-fuzz)
	@if ! rustup toolchain list 2>/dev/null | grep -q '^nightly'; then \
		echo "fuzz: нужен nightly-тулчейн, он не установлен."; \
		echo "      поставьте: rustup toolchain install nightly"; \
		exit 1; \
	fi
	@if ! command -v cargo-fuzz >/dev/null 2>&1; then \
		echo "fuzz: не найден cargo-fuzz."; \
		echo "      поставьте: cargo install cargo-fuzz"; \
		exit 1; \
	fi
	@for target in fuzz_markdown_parser fuzz_typst_escape fuzz_ast_validation; do \
		echo "== $$target: $(FUZZ_TIME)с =="; \
		( cd fuzz && cargo +nightly fuzz run $$target -- -max_total_time=$(FUZZ_TIME) ) || exit 1; \
	done

## --- Обслуживание ------------------------------------------------------------

tools: ## Поставить cargo-deny и cargo-llvm-cov
	$(CARGO) install cargo-deny cargo-llvm-cov

clean: ## Удалить артефакты сборки
	$(CARGO) clean
