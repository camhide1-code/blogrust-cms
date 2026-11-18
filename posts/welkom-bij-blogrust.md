---
title: "1. Welkom bij BlogRust!"
date: "2026-11-10"
weight: 1
category: "Aankondigingen"
tags: ["rust", "cms", "markdown"]
cover: "https://images.unsplash.com/photo-1555066931-4365d14bab8c?auto=format&fit=crop&w=1000&q=80"
summary: "Je eerste blogbericht in BlogRust, geschreven in Markdown met Hugo-stijl frontmatter."
draft: false
---

Welkom op je nieuwe **BlogRust** website! Dit artikel komt rechtstreeks uit het bestand `posts/welkom-bij-blogrust.md`.

## Hoe werkt het Markdown-systeem?

Net als bij **Hugo** begint elk `.md` bestand in de map `posts/` met een zogenaamd *Frontmatter*-blok tussen `---` (of `+++`):

```yaml
---
title: "1. Welkom bij BlogRust!"
date: "2026-11-10"
weight: 1
category: "Aankondigingen"
tags: ["rust", "cms", "markdown"]
summary: "Korte samenvatting voor op de overzichtspagina."
draft: false
---
```

### Voordelen van deze aanpak
- **Supersnel:** Geen zware database-queries nodig voor je teksten.
- **Versiebeheer:** Je kunt al je artikelen makkelijk back-uppen of in Git bewaren.
- **Categorieën & Tags:** Klik bovenaan de pagina op een categorie om direct te filteren!
