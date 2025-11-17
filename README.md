# Bifrost Auth

> [!WARNING]
>
> **This application is still in early Alpha, it is not ready for production usage!**
>
> Currently the backend is still in development, frontend development has yet to begin.
> A production ready release for v0.1.0 is aimed for around January but timelines may change

[![codecov](https://codecov.io/gh/autumn-order/bifrost/graph/badge.svg?token=QGO17OBOST)](https://codecov.io/gh/autumn-order/bifrost)
[![wakatime](https://wakatime.com/badge/github/autumn-order/bifrost.svg)](https://wakatime.com/badge/github/autumn-order/bifrost)
[![Discord](https://img.shields.io/discord/1414000815017824288?logo=Discord&color=%235865F2)](https://discord.gg/HjaGsBBtFg)

An EVE Online authentication platform for coaltions, alliances, and corporations. Built by The Order of Autumn, a corporation part of Black Rose alliance & Phoenix Coalition.

The goal is to create an authentication platform written for high performance, large-scale, and support for multiple corporations & alliances to exist within the same instance or to link their own instance with minimal hassle.
Flexibility should be offered for corporations & alliances to have their own group configurations and discords within an auth instance as determined by the administrators.

## Roadmap

Subject to change, this is a rough draft of goal modules

v0.1.0 Core Functionality
- Auth Module (Completed, currently under testing)
- Job scheduler and worker (In progress, needs refinement & testing)
- Admin (Admin login & controls)
- Groups (Multi-tenant groups owned by auth, other groups, alliances, and corporations)
- Communications - Discord (Support for multiple Discord instances)
- Character Audit (ESI check assets, mails, wallet etc)

v0.2.0 Bifrost Syncing
- Corporation Audit (ESI check assets, wallet, structures)
- Developer API (Provide API endpoints similar to ESI to access Bifrost information for 3rd party apps)
- Syncing (Link bifrost instances together so members don't need to login to multiple auths)

v0.3.0 Alliance/Coalition Essentials
- Recruitment (List corporations accepting applications, create applications to corporations)
- Timerboard
- Doctrines
- Communications - Mumble

v0.4.0 Economy & Industry
- Buyback
- Logistics
- Moon mining
- Market

3rd party extensions to the auth may or may not be possible as Rust is harder than Python/PHP to dynamically modify the code but it has been done. This will be lowest in development priority
and we'll aim to provide first-party support for all of the most utilized features for auths.

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

2. Run tailwindcss

```bash
bunx @tailwindcss/cli -i ./tailwind.css -o ./assets/tailwind.css --watch
```

3. Start the dioxus application in a separate terminal

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

For tests which include redis

### Redis-Related Tests

Redis-related tests include anything involving the worker or scheduler. Including redis-related
tests will increase test execution time by about 5 seconds & requires an active redis instance,
hence why it is feature-gated.

1. Start development docker compose which contains a redis instance

```bash
docker compose -f docker-compose.dev.yml up -d
```

2. Run tests including redis

```bash
cargo test --features redis-test
```

### Code Coverage Report

Generate code coverage report with [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov):
- Use `redis-test` feature to include redis-related tests as well

```bash
cargo llvm-cov --open --features server --ignore-filename-regex "client\/|entity\/|migration\/|bifrost-test-utils\/"
```
