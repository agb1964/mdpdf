# Частые команды mdpdf. `make` без аргументов печатает справку.

CARGO ?= cargo
BIN   := target/release/mdpdf
SAMPLE ?= /tmp/mdpdf-sample.md

.DEFAULT_GOAL := help
.PHONY: help ci fmt fmt-check check clippy test doc deny build release run size \
        coverage fuzz tools clean sample golden-update

help: ## Показать список целей
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

## --- Обязательные проверки (ТЗ §51) -----------------------------------------

ci: fmt-check check clippy test doc deny ## Всё, что гоняет CI

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

run: build ## Запустить на тестовом документе (make run SAMPLE=path.md)
	$(CARGO) run -- --verbose $(SAMPLE)

sample: ## Создать тестовый Markdown (путь в переменной SAMPLE)
	@printf '# Заголовок\n\nТекст с **жирным** и *курсивом*.\n\n- пункт\n- пункт\n' > $(SAMPLE)
	@echo "written $(SAMPLE)"

size: release ## Размер release-бинарника (ТЗ §56)
	@ls -l $(BIN) | awk '{printf "release binary: %s bytes (%.1f MiB)\n", $$5, $$5/1048576}'

## --- Дополнительные проверки -------------------------------------------------

golden-update: ## Перезаписать эталонные AST-, Typst- и PDF-файлы (ТЗ §46, §49)
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test markdown_parser
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test typst_generator
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test golden_pdf pdf_structure_matches_golden_facts

coverage: ## Покрытие тестами, падает ниже 90% (ТЗ §17, §29)
	$(CARGO) llvm-cov --all-features --workspace --summary-only --fail-under-lines 90

FUZZ_TIME ?= 60

fuzz: ## Fuzzing, по $(FUZZ_TIME)с на таргет (ТЗ §50, требует nightly и cargo-fuzz)
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
