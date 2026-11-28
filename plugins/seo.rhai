// 🔍 Smart SEO, Search Engine Sitemap & Social Media Share Plug-in voor BlogRust
// Activeert OpenGraph/Twitter Cards, Schema.org JSON-LD, Social Share knoppen & Google/Bing Sitemap tools.

fn bewerk_navigatie(nav_html) {
    return nav_html + "\n<!-- BLOGRUST_SEO_GLOBAL -->";
}

fn bewerk_bericht(tekst) {
    return tekst + "\n<!-- BLOGRUST_SEO_SHARE -->";
}
