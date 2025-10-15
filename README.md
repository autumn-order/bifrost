# Development

## Prerequisites

Install the following:
- [BunJS](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus](https://dioxuslabs.com/learn/0.6/getting_started/)

### Install Dependencies

Install the tailwindcss dependencies with:

```bash
bun i
```

## Running for Development

Run the following 2 commands in separate terminals:

```bash
bunx @tailwindcss/cli -i ./tailwind.css -o ./assets/tailwind.css --watch
```

```bash
dx serve
```

The application can now be found at `http://localhost:8080`
