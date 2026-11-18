# 🦀 BlogRust CMS — Standaard Editie

Welkom bij **BlogRust**, een razendsnel en modulair Content Management Systeem gebouwd in **Rust** (Axum + SQLite + Argon2), met een **Hugo-stijl Markdown** systeem, **Rhai Plug-in Engine**, **CSS Thema's** en een ingebouwde **Docker Compose & Caddy Generator**.

---

## 📁 Mappenstructuur in deze folder

Wanneer je deze map uitpakt, heb je direct alle bijbehorende mappen klaarstaan:

```text
blogrust-cms/
├── README.md               # Deze handleiding met alle Docker & Rust commando's
├── Cargo.toml              # Rust dependencies (Axum, SQLite, Argon2, Pulldown-cmark, Rhai)
├── Dockerfile              # Multi-stage Docker build (Rust builder -> Debian slim runtime)
├── docker-compose.yml      # Docker Compose stack (BlogRust + Caddy reverse proxy)
├── Caddyfile               # Caddy configuratie (Gzip/Zstd compressie & automatische HTTPS)
├── .dockerignore           # Sluit lokale build-bestanden uit tijdens Docker builds
├── src/
│   └── main.rs             # De Standaard BlogRust CMS broncode (inclusief /instellingen)
├── data/                   # Hier wordt automatisch je SQLite database (blogrust.db) opgeslagen
├── posts/                  # Al je blogartikelen als .md bestanden (Hugo-stijl frontmatter)
├── plugins/                # Rhai (.rhai) plug-ins om artikelen dynamisch te verrijken
├── themes/                 # CSS (.css) thema's voor het uiterlijk van je website
├── media/                  # Geüploade afbeeldingen, banners, logo's en iconen
└── scaffolds/              # Startsjablonen voor nieuwe posts, plug-ins en thema's
```

---

## 🐳 Starten met Docker & Caddy (Aanbevolen)

In deze map staan `Dockerfile`, `docker-compose.yml` en `Caddyfile` al voor je klaar. Alle mappen (`./data`, `./posts`, `./plugins`, `./themes`, `./media` en `./scaffolds`) worden automatisch gekoppeld als persistente volumes.

### 1. Containers bouwen en op de achtergrond starten
Open een terminal (PowerShell, CMD of Bash) in deze map en voer uit:

```bash
docker compose up -d --build
```

- Met de standaard `Caddyfile` draait je blog nu via Caddy op: **`http://localhost`** (poort 80).
- Draai je alleen de BlogRust-container zonder Caddy, dan is je blog bereikbaar op: **`http://localhost:3000`**.

### 2. Live logs bekijken
Wil je zien wat BlogRust of Caddy op de achtergrond doet?

```bash
docker compose logs -f
```

### 3. Containers herstarten (bijv. na het aanpassen van de Caddyfile)
```bash
docker compose restart
```

### 4. Containers stoppen (je data in `./data` en `./posts` blijft veilig bewaard)
```bash
docker compose down
```

---

## 🦀 Alternatief: Lokaal draaien zonder Docker (via Cargo)

Heb je Rust lokaal geïnstalleerd? Dan kun je BlogRust ook rechtstreeks starten:

```bash
cargo run
```

Open daarna **`http://127.0.0.1:3000`** in je browser.

---

## ⚙️ Eerste Installatie (`/setup`) & Instellingen (`/instellingen`)

1. **Eenmalige Setup (`/setup`):**
   Bij het allereerste bezoek opent automatisch de installatiepagina. Kies daar je **Websitetitel**, **Admin-gebruikersnaam** en **Wachtwoord** (wordt veilig gehasht met Argon2id in `data/blogrust.db`).
2. **Instellingenpagina (`/instellingen`):**
   Als je bent ingelogd als Admin kun je via de knop **⚙️ Instellingen** rechtsboven alles aanpassen:
   - **Websitetitel & Ondertitel**
   - **🎨 Website Icoon & Browser-tab Favicon** (kies met 1 klik een emoji of een afbeelding uit `media/`)
   - **🖼️ Website Logo** (kies een logo-afbeelding voor in de bovenbalk met live preview)
   - **🔑 Admin-account & Wachtwoord wijzigen**
   - **🐳 Docker Compose & Caddy Keuzeknop:** Wissel met 1 klik tussen:
     1. *Lokaal met Caddy (`http://localhost` op poort 80)*
     2. *Productie met Caddy & Automatische HTTPS (Let's Encrypt SSL op jouw eigen domein)*
     3. *Alleen BlogRust Container (zonder Caddy, direct op poort 3000)*
