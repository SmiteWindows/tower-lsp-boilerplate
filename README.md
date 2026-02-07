# L Language Server

A Language Server Protocol (LSP) implementation for the L programming language, built with Rust and tower-lsp-server.

> [!note]
> This repo uses [l-lang](https://github.com/IWANABETHATGUY/l-lang), a simple statically-typed programming language with structs, functions, and expressions.

> [!tip]
> If you want a `chumsky` based language implementation, please check out the tag [v1.0.0](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate/tree/v1.0.0)

## A valid program in l-lang

```rust
struct Point {
    x: int,
    y: int,
}

struct Rectangle {
    top_left: Point,
    bottom_right: Point,
}

fn add_points(a: Point, b: Point) -> Point {
    return Point { x: a.x + b.x, y: a.y + b.y };
}

fn main() {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 5, y: 15 };
    let result = add_points(p1, p2);

    let rect = Rectangle {
        top_left: Point { x: 0, y: 100 },
        bottom_right: Point { x: 100, y: 0 },
    };

    return result;
}
```

## Language Features

L-lang is a statically-typed language that supports:

- **Struct definitions** with typed fields
- **Functions** with typed parameters and return types
- **Variable bindings** with type inference
- **Arithmetic operations** (+, -, \*, /)
- **Field access** for structs
- **Struct literals** for instantiation
- **Basic types**: `int`, `bool`, `string`

## Introduction

This repo is a template for building Language Server Protocol (LSP) implementations using `tower-lsp-server`, demonstrating how to create language servers with full IDE support.

## Development using VSCode

1. `bun install`
2. `cargo build`
3. Open the project in VSCode: `code .`
4. In VSCode, press <kbd>F5</kbd> or change to the Debug panel and click <kbd>Launch Extension</kbd>.
5. In the newly launched VSCode instance, open the file `examples/test.l` from this project.
6. If the LSP is working correctly you should see syntax highlighting and the features described below should work.

### Preview and test extension locally with `VsCode`

1. Make sure all dependency are installed.
2. Make sure the `l-language-server` is under your `PATH`
3. `bun run package`
4. `code --install-extension l-language-server-${version}.vsix`, the `version` you could inspect in file system.
5. Restart the `VsCode`, and write a minimal `L` file, then inspect the effect.

For other editor, please refer the related manual, you could skip steps above.

## Features

### Language Features

This Language Server Protocol implementation for l-lang provides comprehensive IDE support with the following features:

### Semantic Tokens

Syntax highlighting based on semantic analysis. Functions, variables, parameters, structs, and fields are highlighted according to their semantic roles.

Make sure semantic highlighting is enabled in your editor settings:

```json
{
  "editor.semanticHighlighting.enabled": true
}
```

### Inlay Hints

Type annotations for variables.

https://github.com/user-attachments/assets/600a2047-a94a-4377-a05e-f11791a17169

### Syntactic and Semantic Error Diagnostics

Real-time error reporting.

https://github.com/user-attachments/assets/2d10070c-340f-4685-965c-2932e16ea20a

### Code Completion

Context-aware suggestions for symbols.

https://github.com/user-attachments/assets/00fed27a-8934-4df6-b001-4da71c3d447c

### Go to Definition

Navigate to symbol declarations.

https://github.com/user-attachments/assets/9a1c3aa1-8f66-4c99-b212-b5356de1d5d2

### Find References

Locate all usages of a symbol.

https://github.com/user-attachments/assets/b71b37aa-4cf9-4433-b408-bd218ba7006c

### Rename

Rename symbols across the entire codebase.

https://github.com/user-attachments/assets/79b3f40b-304d-4cf5-8c6d-ac019eb4090f

### Format

https://github.com/user-attachments/assets/06439fd6-ebf9-414f-86da-95f3b9fa276a

### Hover Information

Show symbol information on hover.

### Document Symbols

Navigate symbols within a document.

### Workspace Symbols

Search for symbols across the workspace.

### Code Actions

Quick fixes and refactoring options.

### Signature Help

Display function signatures.

### Extension Features

- **Status Bar Indicator**: Shows the current status of the language server
- **Output Channel**: Dedicated channel for extension logging
- **Configuration Options**: Customizable settings for the language server
- **Progress Reporting**: Visual feedback for long-running operations
- **Error Handling**: Robust error handling with user-friendly messages
- **Server Restart**: Command to restart the language server

## Installation

### Prerequisites

- [Visual Studio Code](https://code.visualstudio.com/) version 1.107.0 or higher
- [Rust](https://rustup.rs/) (for building the language server)

### Building the Language Server

```bash
# Clone the repository
git clone https://github.com/IWANABETHATGUY/tower-lsp-boilerplate.git
cd tower-lsp-boilerplate

# Build the language server
cargo build --release
```

### Installing the Extension

1. Open VS Code
2. Go to Extensions (Ctrl+Shift+X)
3. Click the "..." menu and select "Install from VSIX..."
4. Select the `l-language-server-1.5.0.vsix` file
5. Restart VS Code

## Configuration

The extension can be configured through VS Code's settings. The following options are available:

- `l-language-server.trace.server`: Controls the level of tracing for the LSP communication
  - `off`: No traces
  - `messages`: Error only
  - `verbose`: Full log
- `l-language-server.maxNumberOfProblems`: Controls the maximum number of problems produced by the server (default: 100)
- `l-language-server.serverPath`: Path to the L language server executable. If empty, the extension will try to find it automatically.

## Usage

1. Create a new file with the `.l` extension
2. Start writing L language code
3. The language server will automatically provide features like completion, diagnostics, and more

## Development

### Project Structure

```
tower-lsp-boilerplate/
├── src/                 # Rust language server implementation
│   └── main.rs         # Main language server code
├── client/             # VS Code extension client
│   ├── src/
│   │   └── extension.ts # Extension entry point
│   └── package.json    # Client dependencies
├── syntaxes/           # TextMate grammar for syntax highlighting
│   └── l.tmLanguage.json
├── language-configuration.json # Language configuration
├── package.json        # Extension manifest
└── Cargo.toml         # Rust dependencies
```

### Building the Extension

```bash
# Install dependencies
cd client && bun install

# Build the extension
bun run compile

# Package the extension
bun run package
```

### Running in Development Mode

1. Open the project in VS Code
2. Press F5 to launch a new VS Code window with the extension loaded
3. Open an `.l` file to test the extension

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- [tower-lsp-server](https://github.com/ebkalibon/tower-lsp) for the LSP framework
- [l-lang](https://github.com/IWANABETHATGUY/l-lang) for the L language implementation
- [VS Code](https://code.visualstudio.com/) for the extension API
