pub const RUST_SAMPLE: &str = r##"use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Server {
    host: String,
    port: u16,
    metadata: HashMap<String, String>,
}

impl Server {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            metadata: HashMap::new(),
        }
    }

    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Starting server on {}:{}", self.host, self.port);
        // Simulate startup
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}
"##;

pub const JS_SAMPLE: &str = r##"import { useState, useEffect } from 'react';

export function useDebounce(value, delay) {
  const [debouncedValue, setDebouncedValue] = useState(value);

  useEffect(() => {
    const handler = setTimeout(() => {
      setDebouncedValue(value);
    }, delay);

    return () => {
      clearTimeout(handler);
    };
  }, [value, delay]);

  return debouncedValue;
}
"##;

pub const PYTHON_SAMPLE: &str = r##"import asyncio
from typing import Dict, Any, Optional
from dataclasses import dataclass

@dataclass
class UserProfile:
    id: int
    username: str
    is_active: bool = True
    preferences: Optional[Dict[str, Any]] = None

async def fetch_user(user_id: int) -> UserProfile:
    """Fetches a user profile from the database."""
    await asyncio.sleep(0.1)  # Simulate network I/O
    return UserProfile(
        id=user_id,
        username=f"user_{user_id}",
        preferences={"theme": "dark"}
    )
"##;

pub const HTML_SAMPLE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Dashboard</title>
    <link rel="stylesheet" href="styles.css">
</head>
<body>
    <nav class="sidebar">
        <ul>
            <li><a href="#home" class="active">Home</a></li>
            <li><a href="#analytics">Analytics</a></li>
            <li><a href="#settings">Settings</a></li>
        </ul>
    </nav>
    <main id="app-root">
        <!-- Content injected via JS -->
    </main>
</body>
</html>
"##;

pub const CSS_SAMPLE: &str = r##":root {
  --primary-color: #3b82f6;
  --bg-color: #0f172a;
  --text-color: #f8fafc;
}

body {
  margin: 0;
  font-family: system-ui, -apple-system, sans-serif;
  background-color: var(--bg-color);
  color: var(--text-color);
}

.sidebar {
  width: 250px;
  height: 100vh;
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(10px);
  border-right: 1px solid rgba(255, 255, 255, 0.1);
}

.sidebar ul {
  list-style: none;
  padding: 1rem;
}
"##;

pub const JSON_SAMPLE: &str = r##"{
  "name": "tuicore-app",
  "version": "1.0.0",
  "description": "A terminal UI application",
  "scripts": {
    "start": "cargo run",
    "test": "cargo test",
    "lint": "cargo clippy -- -D warnings"
  },
  "dependencies": {
    "ratatui": "^0.30.0",
    "crossterm": "^0.28.0"
  },
  "keywords": [
    "tui",
    "rust",
    "terminal"
  ]
}
"##;

pub const TOML_SAMPLE: &str = r##"[package]
name = "tuicore"
version = "0.1.0"
edition = "2021"
authors = ["Developer <dev@example.com>"]
description = "A library-first Rust TUI crate"

[dependencies]
ratatui = { version = "0.30", features = ["widget-calendar"] }
crossterm = "0.29"
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1.0", features = ["full"] }

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
"##;

pub const BASH_SAMPLE: &str = r##"#!/bin/bash
set -euo pipefail

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Error: docker is not installed." >&2
    exit 1
fi

IMAGE_NAME="tuicore-app:latest"

echo "Building Docker image..."
docker build -t "$IMAGE_NAME" .

echo "Running container..."
docker run -d \
    --name tuicore-instance \
    -p 8080:8080 \
    "$IMAGE_NAME"

echo "Deployment complete!"
"##;

pub const MARKDOWN_SAMPLE: &str = r##"# Markdown Showcase

## Headings
# H1 Heading
## H2 Heading
### H3 Heading
#### H4 Heading
##### H5 Heading
###### H6 Heading

---

## Text Formatting
This text is **bold**, this is *italic*, and this is ***bold italic***.
You can also use ~~strikethrough~~ for deleted text.
Here is some `inline code` within a sentence.

## Lists

### Unordered List
* Apples
* Oranges
  * Fuji
  * Navel
    * Seedless

### Ordered List
1. First step
2. Second step
   1. Sub-step 2.1
   2. Sub-step 2.2
3. Third step

### Task List
- [x] Write the code
- [x] Pass the tests
- [ ] Write the documentation

## Blockquotes

> This is a blockquote.
> It can span multiple lines.
>> And it can even be nested!

## Code Blocks

```rust
// A Rust code block
fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```

## Tables

| Syntax      | Description | Alignment |
| :---        |    :----:   |      ---: |
| Header      | Title       | Left      |
| Paragraph   | Text        | Center    |
| Footer      | Bottom      | Right     |

## Links and Images

[Visit Rust's Website](https://www.rust-lang.org/)

![Rust Logo](https://www.rust-lang.org/logos/rust-logo-128x128.png)
"##;
