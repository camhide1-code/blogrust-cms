# 🧩 BlogRust Plug-in Cheat Sheet

Welkom! Je kunt de functionaliteit van BlogRust uitbreiden zonder de Rust-kern te wijzigen.
Plaats simpelweg een `.rhai` bestand in deze map (`plugins/`) of gebruik de ingebouwde editor op **http://127.0.0.1:3000/plugins**.

## 1. Beschikbare Hooks (Functienamen)
Jouw script wordt automatisch aangeroepen als je één of meer van deze functienamen gebruikt:

* `fn bewerk_titel(titel)` -> Draait vlak voordat de artikeltitel wordt getoond.
* `fn bewerk_bericht(tekst)` -> Draait vlak voordat een blogartikel (HTML) op het scherm komt.
* `fn bewerk_bericht_met_cover(tekst, cover, categorie)` -> Draait vlak voor weergave en geeft ook de `cover`-afbeelding en `categorie` uit de Markdown-frontmatter mee.
* `fn bewerk_navigatie(nav_html)` -> Draait op **elke pagina** in de bovenbalk (ideaal voor een globale taalkiezer zoals `i18n.rhai` of extra knoppen/scripts).

## 2. Voorbeeld 1: Woorden vervangen (`highlighter.rhai`)
```javascript
fn bewerk_bericht(tekst) {
    // Vervangt het woord 'BlogRust' automatisch door een dikgedrukte versie
    return tekst.replace("BlogRust", "<strong>🦀 BlogRust</strong>");
}
```

## 3. Voorbeeld 2: Reageren op Categorie of Cover (`tutorial_tip.rhai`)
```javascript
fn bewerk_bericht_met_cover(tekst, cover, categorie) {
    if categorie == "Tutorials" {
        return "<p><strong>💡 Tip:</strong> Volg de stappen hieronder in je eigen tempo!</p>\n" + tekst;
    }
    return tekst;
}
```

## 4. Variabelen uit `.env` uitlezen (`env("SLEUTEL")`)
Beheer je variabelen veilig op `/instellingen` via de `.env` Generator en lees ze in een plug-in (zoals een reactiesysteem) uit met `env("SLEUTEL")`:
```javascript
fn bewerk_bericht(tekst) {
    if env("REACTIES_INGESCHAKELD") == "true" {
        return tekst + "\n<div class='reactie-box'>💬 Reacties zijn geopend!</div>";
    }
    return tekst;
}
```

## 5. Spelregels (Sandbox)
* Jouw script heeft geen toegang tot de server-bestanden of database.
* De wijzigingen zijn direct zichtbaar na het verversen van de browser (geen herstart nodig).
* Op `/plugins` kun je elke plug-in met 1 klik activeren, deactiveren (`.rhai.uit`) of verwijderen.
