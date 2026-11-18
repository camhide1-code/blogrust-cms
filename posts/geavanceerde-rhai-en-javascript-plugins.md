---
title: "4. Geavanceerde Tutorial: Interactieve Plug-ins met Rhai, CSS & JavaScript"
date: "2026-11-13"
weight: 4
category: "Tutorials"
tags: ["rhai", "javascript", "plugins", "geavanceerd", "frontend"]
cover: "/media/blogrust-banner.svg"
summary: "Leer hoe je server-side Rhai combineert met browser-JavaScript en CSS om interactieve widgets, een automatische inhoudsopgave, leesvoortgang-balk en kopieerknoppen te bouwen."
draft: false
---

In de basis-tutorial heb je gezien hoe je met een `.rhai` script eenvoudige tekst kunt aanpassen. Maar wist je dat je vanuit één `.rhai` bestand ook complete **interactieve frontend-widgets** met **CSS (`<style>`)** en **JavaScript (`<script>`)** kunt bouwen?

In deze geavanceerde handleiding duiken we dieper in de samenwerking tussen **Rhai (op de server)** en **JavaScript (in de browser)**.

---

## 1. De Architectuur: Server-side Rhai + Client-side JS

Wanneer een bezoeker een artikel opent op `/post/{slug}`, doorloopt BlogRust deze pijplijn:

1. **Markdown Parser (`pulldown-cmark`):** Zet je `.md` bestand om naar standaard HTML (`<h2>`, `<p>`, `<pre><code>`).
2. **Rhai Engine (Server-side):** Stuurt die HTML-string door jouw actieve `.rhai` scripts in `plugins/`.
3. **Browser Rendering (Client-side):** De browser ontvangt het eindresultaat en voert eventuele `<style>` en `<script>` blokken direct uit.

Dit betekent dat je in één `.rhai` bestand drie lagen kunt combineren:
- **Rhai-logica** (bijv. woorden tellen of controleren in welke `categorie` een post zit).
- **HTML & CSS** (voor de visuele opmaak van je widget).
- **Browser JavaScript** (voor klik-acties, scroll-effecten, `localStorage` of het DOM aanpassen).

---

## 2. Gouden Tip: Omgaan met Aanhalingstekens in Rhai

In een `.rhai` bestand open en sluit je een tekst-string met dubbele aanhalingstekens (`"`). Als je daarbinnen HTML en JavaScript schrijft, houd dan deze vuistregel aan:

- Gebruik **`"`** voor de Rhai-string zelf.
- Gebruik **`'`** (enkele aanhalingstekens) voor HTML-attributen (`class='mijn-box'`) en JavaScript-strings (`querySelector('.content')`).
- Heb je tóch een dubbele quote nodig in JavaScript? Escape hem dan met een backslash: `\"`.

---

## 3. Praktijkvoorbeeld 1: Leesvoortgang-balk (Reading Progress Bar)

Deze plug-in (`leesbalk.rhai`) plaatst helemaal bovenin het scherm een vaste oranje voortgangsbalk die tijdens het scrollen precies laat zien hoe ver de lezer in het artikel is:

```javascript
// Bestand: plugins/leesbalk.rhai
fn bewerk_bericht(tekst) {
    let widget = "
        <style>
            #lees-voortgang {
                position: fixed;
                top: 0;
                left: 0;
                height: 4px;
                background: linear-gradient(90deg, #ea580c, #fb923c);
                width: 0%;
                z-index: 9999;
                transition: width 0.1s ease-out;
            }
        </style>
        <div id='lees-voortgang'></div>
        <script>
            window.addEventListener('scroll', () => {
                const scrollTop = window.scrollY;
                const docHoogte = document.documentElement.scrollHeight - window.innerHeight;
                const percentage = docHoogte > 0 ? (scrollTop / docHoogte) * 100 : 0;
                const balk = document.getElementById('lees-voortgang');
                if (balk) balk.style.width = percentage + '%';
            });
        </script>
    ";

    return widget + tekst;
}
```

---

## 4. Praktijkvoorbeeld 2: Automatische Inhoudsopgave (Table of Contents)

Heb je lange artikelen met meerdere `<h2>` tussenkopjes? Dit script (`inhoudsopgave.rhai`) gebruikt Rhai om te checken of het artikel lang genoeg is, en laat JavaScript vervolgens automatisch alle `<h2>`-koppen verzamelen in een klikbare inhoudsopgave:

```javascript
// Bestand: plugins/inhoudsopgave.rhai
fn bewerk_bericht(tekst) {
    // Alleen tonen als er minstens twee <h2> tussenkoppen in het artikel staan
    if tekst.split("<h2>").len() <= 2 {
        return tekst;
    }

    let toc_script = "
        <div id='auto-toc' style='background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;padding:16px 20px;margin-bottom:24px;'>
            <strong style='display:block;margin-bottom:8px;color:#ea580c;'>📑 Inhoud van dit artikel</strong>
            <ul id='auto-toc-lijst' style='margin:0;padding-left:20px;line-height:1.8;'></ul>
        </div>
        <script>
            (function() {
                const koppen = document.querySelectorAll('.content h2');
                const lijst = document.getElementById('auto-toc-lijst');
                if (!lijst || koppen.length === 0) return;

                koppen.forEach((kop, index) => {
                    const ankerId = 'sectie-' + (index + 1);
                    kop.id = ankerId;
                    kop.style.scrollMarginTop = '24px';

                    const li = document.createElement('li');
                    const link = document.createElement('a');
                    link.href = '#' + ankerId;
                    link.innerText = kop.innerText;
                    link.style.cssText = 'color:#1e293b;text-decoration:none;font-size:14px;';
                    link.onmouseover = () => link.style.color = '#ea580c';
                    link.onmouseout = () => link.style.color = '#1e293b';

                    li.appendChild(link);
                    lijst.appendChild(li);
                });
            })();
        </script>
    ";

    return toc_script + tekst;
}
```

---

## 5. Praktijkvoorbeeld 3: Slimme Codeblokken met "📋 Kopieer" Knop

Voor technische blogs (zoals een Tutorial-categorie) kun je met `bewerk_bericht_met_cover` alleen op bepaalde categorieën een script activeren, of overal een kopieerknop rechtsboven elk `<pre>` codeblok plaatsen:

```javascript
// Bestand: plugins/kopieer_code.rhai
fn bewerk_bericht(tekst) {
    let script = "
        <script>
            document.querySelectorAll('.content pre').forEach((blok) => {
                blok.style.position = 'relative';
                const knop = document.createElement('button');
                knop.innerText = '📋 Kopieer code';
                knop.style.cssText = 'position:absolute;top:10px;right:10px;background:#374151;color:#f9fafb;border:1px solid #4b5563;padding:4px 10px;border-radius:4px;cursor:pointer;font-size:12px;font-family:system-ui,sans-serif;';
                knop.onclick = () => {
                    const codeElement = blok.querySelector('code');
                    const ruweCode = codeElement ? codeElement.innerText : blok.innerText;
                    navigator.clipboard.writeText(ruweCode);
                    knop.innerText = '✅ Gekopieerd!';
                    knop.style.background = '#15803d';
                    setTimeout(() => {
                        knop.innerText = '📋 Kopieer code';
                        knop.style.background = '#374151';
                    }, 1500);
                };
                blok.appendChild(knop);
            });
        </script>
    ";

    return tekst + script;
}
```

---

## 6. Praktijkvoorbeeld 4: Dark Mode / Focus Leesmodus met `localStorage`

Wil je dat lezers het artikel met één druk op de knop in een rustige donkere modus kunnen zetten, én dat de browser hun voorkeur onthoudt voor het volgende artikel? Gebruik dan `localStorage` in je `.rhai` plug-in:

```javascript
// Bestand: plugins/ leesmodus.rhai
fn bewerk_bericht(tekst) {
    let darkmode = "
        <style>
            body.donkere-modus { background: #0f172a !important; color: #e2e8f0 !important; }
            body.donkere-modus .article-box,
            body.donkere-modus .topbar { background: #1e293b !important; border-color: #334155 !important; color: #e2e8f0 !important; }
            body.donkere-modus h1,
            body.donkere-modus h2,
            body.donkere-modus h3 { color: #f8fafc !important; border-color: #334155 !important; }
            body.donkere-modus .content code { background: #334155 !important; color: #f8fafc !important; }
        </style>
        <div style='display:flex;justify-content:flex-end;margin-bottom:12px;'>
            <button id='modus-knop' onclick='wisselLeesmodus()' style='background:#f1f5f9;border:1px solid #cbd5e1;color:#334155;padding:6px 12px;border-radius:6px;cursor:pointer;font-size:13px;font-weight:600;'>
                🌙 Donkere leesmodus
            </button>
        </div>
        <script>
            function pasModusToe(donker) {
                const knop = document.getElementById('modus-knop');
                if (donker) {
                    document.body.classList.add('donkere-modus');
                    if (knop) knop.innerText = '☀️ Lichte leesmodus';
                } else {
                    document.body.classList.remove('donkere-modus');
                    if (knop) knop.innerText = '🌙 Donkere leesmodus';
                }
            }
            function wisselLeesmodus() {
                const isNuDonker = !document.body.classList.contains('donkere-modus');
                localStorage.setItem('blogrust_donker', isNuDonker ? '1' : '0');
                pasModusToe(isNuDonker);
            }
            pasModusToe(localStorage.getItem('blogrust_donker') === '1');
        </script>
    ";

    return darkmode + tekst;
}
```

---

## Zelf uitproberen?

1. Kopieer één van de bovenstaande scripts.
2. Klik rechtsboven in de balk op **🧩 Plug-ins** (`/plugins`).
3. Plak de code onderaan bij **🛠️ Eigen Rhai Plug-in Schrijven & Installeren**, geef hem een naam (bijv. `inhoudsopgave.rhai`) en klik op **Opslaan**.
4. Open dit artikel opnieuw en zie je nieuwe JavaScript-plug-in direct in actie! 🦀
