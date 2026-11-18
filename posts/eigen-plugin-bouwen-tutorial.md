---
title: "3. Tutorial: Zo bouw je zelf een Plug-in voor BlogRust (.rhai)"
date: "2026-11-12"
weight: 3
category: "Tutorials"
tags: ["plugins", "rhai", "handleiding", "rust"]
cover: "/media/blogrust-banner.svg"
summary: "Leer stap voor stap hoe je in 2 minuten je eigen Rhai plug-in schrijft en installeert zonder de Rust-broncode aan te passen."
draft: false
---

Eén van de krachtigste functies van **BlogRust** is het ingebouwde plug-insysteem op basis van **Rhai** (een veilige scripting-taal voor Rust). Hiermee kun je de weergave van je blogartikelen en titels uitbreiden zonder dat je de server hoeft te herstarten of opnieuw hoeft te compileren!

---

## Hoe werkt een BlogRust Plug-in?

Elke plug-in is een simpel tekstbestand met de extensie `.rhai` in de map `plugins/`. Zodra een bezoeker een artikel opent, voert BlogRust automatisch alle actieve `.rhai` scripts uit.

Je kunt een plug-in op **twee manieren** toevoegen:
1. **Via de browser (Aanbevolen):** Ga rechtsboven naar **🧩 Plug-ins** (`/plugins`), scroll naar onderen naar *Eigen Rhai Plug-in Schrijven & Installeren*, kies een naam (bijv. `mijn_plugin.rhai`), plak je code en klik op **Opslaan**.
2. **Via je code-editor:** Maak rechtstreeks een nieuw `.rhai` bestand aan in de map `plugins/`.

---

## De 3 Beschikbare Hooks (Functies)

BlogRust zoekt in jouw `.rhai` bestand naar drie vaste functienamen (*hooks*):

| Functie (Hook) | Wanneer draait deze? | Wat krijgt hij mee? |
| :--- | :--- | :--- |
| `fn bewerk_titel(titel)` | Vlak voordat de titel van een post wordt getoond | De originele `titel` als tekst |
| `fn bewerk_bericht(tekst)` | Vlak voordat de HTML-inhoud van een post op het scherm komt | De gerenderde `tekst` (HTML) |
| `fn bewerk_bericht_met_cover(tekst, cover, categorie)` | Vlak voor weergave, inclusief toegang tot cover & categorie | `tekst`, `cover`-URL en `categorie` |

---

## Voorbeeld 1: Automatische Emoji's & Woorden Vervangen

Stel dat je wilt dat het woord `Rust` in elk artikel automatisch wordt vervangen door `🦀 Rust`. Maak een plug-in genaamd `rust_emoji.rhai`:

```javascript
// Vervangt het woord Rust automatisch door een krab-emoji + dikgedrukte tekst
fn bewerk_bericht(tekst) {
    return tekst.replace("Rust", "<strong>🦀 Rust</strong>");
}
```

---

## Voorbeeld 2: Een Categorie-Waarschuwing of Banner Toevoegen

Met `bewerk_bericht_met_cover(tekst, cover, categorie)` kun je slim reageren op de categorie van een artikel. Bijvoorbeeld om bovenaan elke **Tutorials**-post een handige tip-balk te tonen:

```javascript
// Toont automatisch een tip-box bovenaan elk artikel in de categorie 'Tutorials'
fn bewerk_bericht_met_cover(tekst, cover, categorie) {
    if categorie == "Tutorials" {
        let banner = "<div style=\"background:#eff6ff;border-left:4px solid #2563eb;padding:12px 16px;margin-bottom:20px;border-radius:4px;\">💡 <strong>Tip:</strong> Dit is een stap-voor-stap handleiding. Je kunt de codeblokken direct kopiëren!</div>\n";
        return banner + tekst;
    }
    return tekst;
}
```

---

## Voorbeeld 3: De Titel Aanpassen

Wil je achter elke artikeltitel automatisch een vinkje of label zetten? Gebruik dan `bewerk_titel`:

```javascript
fn bewerk_titel(titel) {
    return titel + " ✨";
}
```

---

## Veiligheid (Sandbox)

Alle `.rhai` plug-ins draaien in een afgeschermde **Sandbox**:
- Een plug-in kan **geen** bestanden op je harde schijf wissen of wijzigen.
- Een plug-in heeft **geen** toegang tot de wachtwoorden in je SQLite-database.
- Werkt een plug-in niet zoals verwacht? Ga naar **🧩 Plug-ins** (`/plugins`) en klik op **⏸️ Deactiveren** of **🗑️ Verwijder**.
