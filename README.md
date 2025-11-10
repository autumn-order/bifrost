
# Deployment

## Prerequisites

### Install Dependencies

- [Docker](https://docs.docker.com/engine/install/)
- [git](https://git-scm.com/install/linux)

### Clone the repository

```bash
git clone https://github.com/autumn-order/bifrost
```

### Create EVE Online Developer Application

Create a developer application at <https://developers.eveonline.com/applications>

- Set callback URL to `https://your.domain.com/api/auth/callback`
- Enable ALL scopes for the application

### Configure Environment Variables

```bash
cp .env.example .env
```

Set the following in `.env`:

- `DOMAIN` (Set to your domain, e.g. `bifrost.autumn-order.com`)
- `CONTACT_EMAIL` (Email for EVE developers to contact you if any issues occur)
- `ESI_CLIENT_ID` (Get from <https://developers.eveonline.com/applications>)
- `ESI_CLIENT_SECRET`(Get from from <https://developers.eveonline.com/applications>)
- `ESI_CALLBACK_URL` (This will be what you set in your dev application `https://your.domain.com/api/auth/callback`)
- `POSTGRES_PASSWORD` (Set to a secure password)

## Running for Production

1. Start traefik proxy instance

```bash
sudo docker network create traefik
```

```bash
sudo docker compose -f docker-compose.traefik.yml up -d
```

2. Run Bifrost

```bash
sudo docker compose up -d
```

This will take a few minutes to build depending on server resources, this will only occur on first startup or after updating

## Updating

1. Pull repository changes

```bash
git pull
```

2. Rebuild the application with the `--build` flag

```bash
sudo docker compose up -d --build
```

# Development

## Prerequisites

### Install Dependencies

- [BunJS](https://bun.sh/)
- [Rust](https://www.rust-lang.org/tools/install)
- [Dioxus](https://dioxuslabs.com/learn/0.7/getting_started/)
- [Docker](https://docs.docker.com/engine/install/)

Install the tailwindcss dependencies with:

```bash
bun i
```

### Create EVE Online Developer Application

Create a developer application at <https://developers.eveonline.com/applications>

- For development set callback URL to `http://localhost:8080/api/auth/callback`
- Enable all scopes for the application

### Configure Environment Variables

```bash
cp .env.example .env
```

Set the following in `.env`:

- `DOMAIN` (Unnecessary for testing on localhost, ignore it)
- `CONTACT_EMAIL` (Email for EVE developers to contact you if any issues occur)
- `ESI_CLIENT_ID` (Get from <https://developers.eveonline.com/applications>)
- `ESI_CLIENT_SECRET`(Get from from <https://developers.eveonline.com/applications>)
- `ESI_CALLBACK_URL` (For development, this will be `http://localhost:8080/api/auth/callback`)
- `POSTGRES_PASSWORD` (Set to a secure password)
- `DATABASE_URL` (Replace the `POSTGRES_PASSWORD` within the `DATABASE_URL` to the password you set)

## Running for Development

1. Start the development Postgres instance with:

```bash
docker compose -f docker-compose.dev.yml up -d
```

2. Start the dioxus application

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

You can then find the API docs at `http://localhost:8080/api/docs`

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

Generate code coverage report with [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):
```bash
cargo llvm-cov --open --features server --ignore-filename-regex "client\/|entity\/|migration\/|bifrost-test-utils\/"
```
