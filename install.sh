#!/bin/bash
set -e

# UHPM Installer Script
# Устанавливает UHPM и связанные утилиты в домашнюю директорию

# 1. Проверка наличия Rust и Cargo
if ! command -v cargo &> /dev/null; then
    echo "Rust и Cargo не установлены. Установите Rust: https://www.rust-lang.org/tools/install"
    exit 1
fi

# 2. Создание необходимых директорий
UHPM_DIR="$HOME/.uhpm"
PACKAGES_DIR="$UHPM_DIR/packages"
TMP_DIR="$UHPM_DIR/tmp"

mkdir -p "$PACKAGES_DIR" "$TMP_DIR"

# 3. Сборка проекта
echo "Сборка UHPM и утилит..."
cargo build --release

# 4. Установка бинарников
BIN_DIR="$HOME/.local/bin"
mkdir -p "$BIN_DIR"

echo "Установка бинарников в $BIN_DIR..."
cp target/release/uhpm "$BIN_DIR/"
cp target/release/uhpmk "$BIN_DIR/"
if [ -f target/release/uhprepo ]; then
    cp target/release/uhprepo "$BIN_DIR/"
fi

# 5. Проверка установки
echo "Проверка установки..."
for bin in uhpm uhpmk uhprepo; do
    if command -v "$BIN_DIR/$bin" &> /dev/null; then
        echo "$bin установлен успешно: $($BIN_DIR/$bin --version || echo 'версия недоступна')"
    else
        echo "Ошибка установки $bin"
    fi
done

echo "Установка завершена. UHPM готов к использованию."
