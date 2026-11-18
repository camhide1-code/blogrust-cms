---
title: "5. Tutorial: Zo maak en installeer je een eigen Thema in BlogRust (.css)"
date: "2026-11-14"
weight: 5
category: "Tutorials"
tags: ["themes", "css", "design", "handleiding"]
cover: "/media/blogrust-banner.svg"
summary: "Ontdek hoe je met één klik van thema wisselt op /themes of zelf een eigen .css thema bouwt voor je BlogRust website."
draft: false
---

Net als bij het plug-insysteem (`plugins/`) heeft **BlogRust** ook een modulair **Thema-systeem** in de map `themes/`. Hiermee verander je met één muisklik de kleuren, typografie, achtergronden en kaarten van je hele blog!

---

## 1. Hoe werkt de Thema-pagina (`/themes`)?

Wanneer je bent ingelogd als beheerder, zie je rechtsboven in de balk de knop **🎨 Thema's** (`/themes`). Op die pagina kun je:

1. **Wisselen van Actief Thema:** Klik bij een geïnstalleerd thema op **🎨 Activeer** en je hele blog krijgt direct dat uiterlijk.
2. **1-Klik Thema's Installeren uit de Bibliotheek:** Kies uit kant-en-klare thema's zoals *🌌 Midnight Dark*, *📰 Klassieke Krant* of *🌲 Bosgroen Minimal* om ze automatisch in de map `themes/` te plaatsen.
3. **Zelf een Thema Schrijven:** Onderaan `/themes` kun je je eigen `.css` thema intypen, opslaan en meteen activeren.

---

## 2. Welke CSS-klassen kun je aanpassen?

Elk onderdeel van BlogRust heeft een duidelijke CSS-klasse die je in je `.css` bestand kunt overschrijven:

| CSS Selector | Onderdeel op de site |
| :--- | :--- |
| `body` | Algemene achtergrondkleur, tekstkleur en lettertype (`font-family`) |
| `.topbar` | De bovenste navigatiebalk |
| `.brand` | Het logo + de sitetitel linksboven |
| `.hero` | Het introductieblok bovenaan de startpagina |
| `.post-card` | De losse artikel-kaartjes op de startpagina |
| `.article-box` | Het leesvlak van een geopend artikel (`/post/{slug}`) |
| `.cat-pill.actief` | De geselecteerde categorie-knop |
| `.cat-badge` & `.tag` | Categorie- en `#tag`-labels |

---

## 3. Voorbeeld: Zelf een "Oceaan Blauw" Thema bouwen

Plak de onderstaande code op `/themes` onder **🛠️ Eigen CSS Thema Schrijven & Installeren** met de naam `oceaan.css`:

```css
/* Oceaan Blauw Thema - Frisse blauwe kleuren en zachte schaduwen */
body {
    background: #f0f9ff !important;
    color: #0f172a !important;
}

.topbar, .hero, .post-card, .article-box, .card {
    background: #ffffff !important;
    border-color: #bae6fd !important;
    border-radius: 12px !important;
}

.brand, .hero h1, .read-more, .back-link {
    color: #0284c7 !important;
}

.cat-pill.actief, .btn-new {
    background: #0284c7 !important;
    border-color: #0284c7 !important;
    color: #ffffff !important;
}

.cat-badge {
    background: #e0f2fe !important;
    color: #0369a1 !important;
    border-color: #7dd3fc !important;
}
```

> **Tip:** Zet op de allereerste regel van je `.css` bestand een korte toelichting tussen `/* ... */`. BlogRust leest die eerste regel automatisch uit en toont hem als beschrijving op de **🎨 Thema's** pagina!
