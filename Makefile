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

golden-update: ## Перезаписать эталонные AST-файлы (ТЗ §46)
	MDPDF_UPDATE_GOLDEN=1 $(CARGO) test --test markdown_parser

coverage: ## Покрытие тестами (ТЗ §17, §29: цель — 90%)
	$(CARGO) llvm-cov --all-features --workspace --summary-only

fuzz: ## Fuzzing (ТЗ §50, требует nightly и cargo-fuzz)
	@echo "fuzz targets появятся на Milestone 5 (ТЗ §50)"

## --- Обслуживание ------------------------------------------------------------

tools: ## Поставить cargo-deny и cargo-llvm-cov
	$(CARGO) install cargo-deny cargo-llvm-cov

clean: ## Удалить артефакты сборки
	$(CARGO) clean
