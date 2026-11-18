---
title: "6. BlogRust Deployen met Docker Compose & Caddy (HTTPS)"
date: "2026-11-15"
weight: 6
category: "Tutorials"
tags: ["docker", "caddy", "compose", "deploy", "https"]
cover: "/media/blogrust-banner.svg"
summary: "Genereer met één druk op de knop een complete Dockerfile, docker-compose.yml en Caddyfile om BlogRust in containers te draaien."
draft: false
---

Wil je jouw **BlogRust** website in een geïsoleerde container draaien (lokaal of op een eigen VPS-server met een echte domeinnaam en automatische HTTPS)?

Via **⚙️ Instellingen** (`/instellingen`) vind je nu onderaan de **🐳 Docker & Caddy Compose Generator**!

---

## 1. Hoe werkt de Keuzeknop onder Instellingen?

Op de pagina **⚙️ Instellingen** kun je kiezen uit drie container-profielen:

1. **Lokaal met Caddy (`localhost`)**  
   Draait zowel BlogRust als de Caddy webserver in Docker Compose. Je bereikt je site op `http://localhost` (poort 80), terwijl Caddy zorgt voor Gzip/Zstd-compressie.
2. **Productie met Caddy + Automatische HTTPS (Domeinnaam)**  
   Je vult je eigen domeinnaam in (bijvoorbeeld `mijnblog.nl`). Caddy vraagt dan volledig automatisch een gratis **Let's Encrypt SSL-certificaat** aan en stuurt al het verkeer veilig door naar `blogrust:3000`.
3. **Alleen BlogRust Container (Zonder Caddy)**  
   Handig als je zelf al een eigen reverse-proxy (zoals Nginx of Traefik) op je server hebt draaien.

Zodra je op **🐳 Genereer Docker & Caddy Bestanden** klikt, worden deze drie bestanden automatisch in de hoofdmap van je project klaargezet:
- `Dockerfile` (Multi-stage Rust build)
- `docker-compose.yml` (Container-orkestratie met persistente mappen)
- `Caddyfile` (Configuratie voor de Caddy reverse-proxy)

---

## 2. Al je data blijft veilig bewaard (Volumes)

In de gegenereerde `docker-compose.yml` worden al je belangrijke mappen automatisch gekoppeld (*gemount*) aan de container:

```yaml
volumes:
  - ./data:/app/data         # Je SQLite database (blogrust.db)
  - ./posts:/app/posts       # Al je Markdown artikelen (.md)
  - ./plugins:/app/plugins   # Al je Rhai plug-ins (.rhai)
  - ./themes:/app/themes     # Al je CSS thema's (.css)
  - ./media:/app/media       # Al je geüploade afbeeldingen
  - ./scaffolds:/app/scaffolds
```

Hierdoor gaat er **nooit** een bericht, foto, thema of database verloren wanneer je de container opnieuw opstart of bijwerkt!

---

## 3. De Containers Starten & Stoppen

Open een terminal in je projectmap (`C:\projects\Blogrust`) en voer dit commando uit:

```bash
# Bouw en start de containers op de achtergrond
docker compose up -d --build
```

Wil je de logs bekijken of de containers weer stoppen?

```bash
# Bekijk live de logs van BlogRust en Caddy
docker compose logs -f

# Stop de containers (je data in ./data en ./posts blijft gewoon bewaard!)
docker compose down
```
