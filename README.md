# 🌌 Galera

**Galera** is a FLOSS and self-hosted, lightweight, high-performance image gallery built with **Rust**.

## ⚠️ Caution

Galera is a **Work In Progress software** 🏗️.

🔄 Changes are expected.
🖼️ No image transcoding for now.

## ✨ Key Features

- 📖 **Open-source:** Free/libre and open-source software; ready to be self-hosted.
- 🦀 **Rust-powered:** Blazing fast with memory safety.
- ☁️ **Modular:** Designed as a modular service for easy scalability.
- 🪶 **Low Resource:** Minimalist runtime, ideal for cost-effective self-hosting. Galera uses only **~3.3 MiB RAM** in idle (`cargo run --release`).
- 🛡️ **SSO:** Support for OpenID Connect 1.0 and OpenID Connect RP-Initiated Logout 1.0.

---

## 🚀 Quick Start

Looking for advanced setups or in more depth? See the [individual example docker-compose.yml files](https://github.com/galera-org/meta).

### 1. Environment Configuration

Choose the appropriate template and rename it to `.env`:

- **For local development:** `cp .env.dev-example .env`
- **For production:** `cp .env.prod-example .env`. Production template expects galera behind a reverse proxy with https.

Change the .env file to suit your needs.

> **Note:** Explicitly setting your `.env` is recommended for production security (e.g. `DISABLE_LOCAL_AUTH=true` or `DISABLE_LOCAL_SIGNUPS=true`).

### 2. Docker Deployment

Create a `docker-compose.yml`:

```yaml
services:
  galera-mariadb:
    image: mariadb:12.1
    container_name: galera-mariadb
    volumes:
      - ./galera/db:/var/lib/mysql
    environment:
      - MYSQL_DATABASE=galera
      - MYSQL_ROOT_PASSWORD=rootpassword
      - MYSQL_PASSWORD=password
      - MYSQL_USER=docker
    healthcheck:
      test: ["CMD", "healthcheck.sh", "--connect", "--innodb_initialized"]
      start_period: 1m
      interval: 5s
      timeout: 5s
      retries: 10
    ports:
      - 3306:3306
    restart: unless-stopped

  galera:
    image: ghcr.io/itzbobocz/galera:unstable
    container_name: galera
    depends_on:
      galera-mariadb:
        condition: service_healthy
    env_file: .env
    volumes:
      - ./galera/config:/root/.config/galera
      - ./galera/data:/root/.local/share/galera
    healthcheck:
      test: ["CMD-SHELL", "bash -lc 'exec 3<>/dev/tcp/127.0.0.1/8000'"]
      interval: 10s
      retries: 5
      start_period: 5s
      timeout: 2s
    ports:
      - 8000:8000
    restart: unless-stopped

  galera-web:
    image: ghcr.io/itzbobocz/galera-web:unstable
    container_name: galera-web
    depends_on:
      galera:
        condition: service_healthy
    environment:
      - BACKEND_URL=http://galera:8000
    ports:
      - 3000:80
    restart: unless-stopped
```

```bash
docker compose up -d
```

### 🛠️ Development

1. Install Rust: Use `rustup` to get the latest stable toolchain.

2. Install MariaDB locally or via Docker.

3. Copy and set up .env: `cp .env.dev-example .env`

4. Run:

```bash
cargo run
```

Backend starts on http://localhost:8000.
