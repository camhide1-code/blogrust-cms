// 🌍 Multi-Language (i18n) Plug-in voor BlogRust (Standaard: Engels + Browser API)
// Beschermde merknamen (worden nooit vertaald naar 'Rest' of 'Ruhe'):
// BlogRust, Blogrust, Rust, Rhai, Hugo, Caddy, Axum, Argon2, SQLite, Markdown, Frontmatter
fn bewerk_navigatie(nav_html) {
    let standaard_taal = env("DEFAULT_LANG");
    if standaard_taal == "" {
        standaard_taal = "en";
    }
    return nav_html + "\n<!-- BLOGRUST_I18N:" + standaard_taal + " -->";
}
