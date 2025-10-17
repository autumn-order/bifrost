# Development

## Prerequisites

### Install Dependencies
- [BunJS](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus](https://dioxuslabs.com/learn/0.6/getting_started/)

Install the tailwindcss dependencies with:

```bash
bun i
```

### Create EVE Online Developer Application

Create a developer application at <https://developers.eveonline.com/applications>

- For development set callback URL to `http://localhost:8080/auth/callback`

### Configure Environment Variables

```bash
cp .env.example .env
```

Set the following in `.env`:

- `CONTACT_EMAIL`
- `ESI_CLIENT_ID` (Get from <https://developers.eveonline.com/applications>)
- `ESI_CLIENT_SECRET`(Get from from <https://developers.eveonline.com/applications>)
- `ESI_CALLBACK_URL` (For development, this will be `http://localhost:8080/auth/callback`)

## Running for Development

Run the following 2 commands in separate terminals:

```bash
bunx @tailwindcss/cli -i ./tailwind.css -o ./assets/tailwind.css --watch
```

```bash
dx serve
```

The application can now be found at `http://localhost:8080`

## Testing

Run tests with:

```bash
cargo test --features server
```
