# Nexus Code

Editor de texto para Linux construído com [Iced](https://iced.rs) e [scrive_iced](https://crates.io/crates/scrive-iced), com realce de sintaxe e suporte a múltiplas abas.

## Funcionalidades

- Múltiplas abas de edição.
- Abertura e salvamento de arquivos.
- Realce de sintaxe (gramática Sublime Text, ex.: Rust).
- Temas claro/escuro (Nexus Light e Scrice Dark).
- Personalização de tamanho de fonte e imagem de fundo.
- Interface nativa (X11/Wayland) via iced.

## Requisitos

- Rust (edição 2024) e Cargo.
- Dependências do sistema para iced (X11/Wayland, conforme a distribuição).

## Compilar e executar

```bash
cargo build --release
./target/release/rust-text-editor
```

Para desenvolvimento:

```bash
cargo run
```

## Estrutura

- `src/main.rs` — aplicação principal (Iced).
- `assets/` — gramáticas (`.sublime-syntax`) e temas (`.tmTheme`).
- `vendor/scrive-iced/` — componente de editor de código (vendored).

## Licença

Uso pessoal. (Ajuste conforme sua preferência.)
