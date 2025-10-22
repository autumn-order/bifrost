# Development

## Prerequisites

### Install Dependencies

- [BunJS](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus](https://dioxuslabs.com/learn/0.6/getting_started/)
- [Docker](https://docs.docker.com/engine/install/)

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
- `POSTGRES_PASSWORD` (Set to a secure password)
- `DATABASE_URL` (Replace the `POSTGRES_PASSWORD` within the `DATABASE_URL` to the password you set)

## Running for Development

1. Start the development Postgres instance with:

```bash
docker compose -f docker-compose.dev.yml up -d
```

2. Run the following 2 commands in separate terminals:

```bash
bunx @tailwindcss/cli -i ./tailwind.css -o ./assets/tailwind.css --watch
```

```bash
dx serve
```

The application can now be found at `http://localhost:8080`

## Database Migrations

Ensure the development postgres instance is running first:

```bash
docker compose -f docker-compose.dev.yml up -d
```

1. Apply migrations to the database

```bash
sea-orm-cli migrate
```

2. Generate entities based upon database tables applied by the migration

```bash
sea-orm-cli generate entity -o ./entity/src/entities/ --date-time-crate chrono
```

### Additionally Useful DB Commands

Drop all tables & reapply migrations
- Use this if you modified migrations and need a fresh start

```bash
sea-orm-cli fresh
```

Rollback all applied migrations & reapply them
- Use this to ensure both your up & down methods of your migrations work

```bash
sea-orm-cli migrate refresh
```

## Testing

Run tests for the server with:

```bash
cargo test --features server
```
