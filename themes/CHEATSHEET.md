# 🎨 BlogRust Thema Cheat Sheet

Welkom! Je kunt het volledige uiterlijk van BlogRust aanpassen door een `.css` bestand in de map `themes/` te plaatsen, of via de thema-editor op **http://127.0.0.1:3000/themes**.

## 1. Belangrijkste CSS-klassen om te stylen
Elk pagina-onderdeel in BlogRust heeft een vaste, voorspelbare CSS-klasse:

* `body` -> Achtergrondkleur, standaard tekstkleur en lettertype (`font-family`).
* `.topbar` -> De navigatiebalk bovenaan de pagina.
* `.brand` -> Het logo en de sitetitel linksboven.
* `.hero` -> De welkomst-header op de startpagina (`/`).
* `.cat-pill` & `.cat-pill.actief` -> De categorie-filterknoppen op de startpagina.
* `.post-card` -> Een artikelkaartje in het overzicht op de startpagina.
* `.article-box` -> De volledige artikelcontainer op `/post/{slug}`.
* `.cat-badge` & `.tag` -> De categorie- en tag-labels bij elk artikel.

## 2. Voorbeeld Thema (`themes/mijn_thema.css`)
De eerste regel (`/* ... */`) wordt automatisch als beschrijving getoond op de `/themes` pagina:

```css
/* Mijn Eigen Paarse Thema - Modern thema met paarse accenten */
body {
    background: #f5f3ff !important;
    color: #1e1b4b !important;
}
.topbar, .hero, .post-card, .article-box, .card {
    background: #ffffff !important;
    border-color: #ddd6fe !important;
}
.brand, .hero h1, .read-more, .back-link {
    color: #7c3aed !important;
}
.cat-pill.actief, .btn-new {
    background: #7c3aed !important;
    border-color: #7c3aed !important;
    color: #ffffff !important;
}
```
