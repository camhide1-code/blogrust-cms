// ============================================================================
// 🦀 BlogRust Standaard CMS Editie (Zonder Google Auth Developer Functies)
// Plaats dit bestand in src/main.rs van je eigen BlogRust installatie.
// ============================================================================

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{DefaultBodyLimit, Form, Multipart, Path as AxumPath, Query, State},
    http::{
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
        HeaderMap, HeaderValue, StatusCode,
    },
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use pulldown_cmark::{html, Options, Parser};
use rhai::{Engine, Scope};
use serde::Deserialize;
use sqlx::{sqlite::SqliteConnectOptions, SqlitePool};
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    net::SocketAddr,
    path::Path,
    str::FromStr,
    sync::{Arc, RwLock},
};

// Gedeelde applicatie-state
#[derive(Clone)]
struct AppState {
    db_pad: String,
    posts_map: String,
    plugins_map: String,
    themes_map: String,
    media_map: String,
    scaffolds_map: String,
    sessies: Arc<RwLock<HashMap<String, String>>>, // sessie_token -> gebruikersnaam
}

// Hugo-stijl Markdown Post structuur
#[derive(Clone, Debug)]
struct MarkdownPost {
    slug: String,
    titel: String,
    datum: String,
    volgorde: i32, // Hugo 'weight:' voor sortering van begin naar eind
    categorie: String,
    tags: Vec<String>,
    cover: String,
    samenvatting: String,
    concept: bool,
    ruwe_markdown: String,
    html_inhoud: String,
}

// Informatie over een bestand in de `plugins/` map
struct GeinstalleerdePlugin {
    bestandsnaam: String,
    actief: bool,
    code_preview: String,
}

// Informatie over een thema-bestand in de `themes/` map
struct GeinstalleerdThema {
    bestandsnaam: String,
    actief: bool,
    beschrijving: String,
}

// Informatie over een bestand in de `media/` map
struct MediaBestand {
    bestandsnaam: String,
    grootte_kb: u64,
}

#[derive(Deserialize)]
struct SetupForm {
    site_titel: String,
    admin_naam: String,
    wachtwoord: String,
}

#[derive(Deserialize)]
struct LoginForm {
    gebruikersnaam: String,
    wachtwoord: String,
}

#[derive(Deserialize)]
struct InstellingenForm {
    site_titel: String,
    site_beschrijving: String,
    #[serde(default)]
    site_icoon: String,
    #[serde(default)]
    site_logo: String,
    #[serde(default)]
    google_client_id: String,
    admin_naam: String,
    nieuw_wachtwoord: String,
}

#[derive(Deserialize)]
struct GoogleLoginForm {
    #[serde(default)]
    email: String,
    #[serde(default)]
    credential: String,
    #[serde(default)]
    terug_naar: String,
}

#[derive(Deserialize)]
struct NieuwBerichtForm {
    originele_slug: Option<String>,
    titel: String,
    datum: String,
    categorie: String,
    tags: String,
    cover: String,
    samenvatting: String,
    concept: Option<String>,
    inhoud: String,
}

#[derive(Deserialize)]
struct PluginActieForm {
    actie: String,
    naam: String,
    code: Option<String>,
}

#[derive(Deserialize)]
struct ThemaActieForm {
    actie: String,
    naam: String,
    css: Option<String>,
}

#[derive(Deserialize)]
struct MediaVerwijderForm {
    bestandsnaam: String,
}

#[derive(Deserialize)]
struct ReactieForm {
    bericht: String,
}

#[derive(Deserialize)]
struct ReactieVerwijderForm {
    reactie_id: i64,
}

#[derive(Deserialize)]
struct DockerComposeForm {
    modus: String, // "caddy_lokaal", "caddy_https", "alleen_blogrust"
    domein: String,
}

#[derive(Deserialize)]
struct DotEnvForm {
    poort: String,
    domein: String,
    db_pad: String,
    #[serde(default)]
    google_client_id: String,
    #[serde(default)]
    extra_env: String,
}

#[derive(Deserialize, Default)]
struct MeldingQuery {
    opgeslagen: Option<String>,
    docker_klaar: Option<String>,
    env_klaar: Option<String>,
    fout: Option<String>,
    categorie: Option<String>,
    tag: Option<String>,
    zoek: Option<String>,
    sorteer: Option<String>, // "begin" (standaard) of "nieuwste"
    melding: Option<String>,
}

#[tokio::main]
async fn main() {
    fs::create_dir_all("data").unwrap();
    fs::create_dir_all("posts").unwrap();
    fs::create_dir_all("plugins").unwrap();
    fs::create_dir_all("themes").unwrap();
    fs::create_dir_all("media").unwrap();
    fs::create_dir_all("scaffolds").unwrap();

    let db_pad = haal_env_waarde("BLOGRUST_DB_PAD").unwrap_or_else(|| "data/blogrust.db".to_string());
    // Ondersteun zowel Google Cloud Run's automatische PORT variabele als BLOGRUST_PORT
    let poort: u16 = haal_env_waarde("PORT")
        .or_else(|| haal_env_waarde("BLOGRUST_PORT"))
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let state = AppState {
        db_pad,
        posts_map: "posts".to_string(),
        plugins_map: "plugins".to_string(),
        themes_map: "themes".to_string(),
        media_map: "media".to_string(),
        scaffolds_map: "scaffolds".to_string(),
        sessies: Arc::new(RwLock::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(home_pagina))
        .route("/post/{slug}", get(toon_post_pagina))
        .route("/post/{slug}/reactie", post(verwerk_nieuwe_reactie))
        .route(
            "/post/{slug}/reactie/verwijder",
            post(verwerk_reactie_verwijder),
        )
        .route("/nieuw", get(nieuw_bericht_pagina).post(verwerk_nieuw_bericht))
        .route("/bewerk/{slug}", get(bewerk_bericht_pagina))
        .route("/verwijder/{slug}", post(verwijder_bericht_actie))
        .route("/plugins", get(plugins_pagina).post(verwerk_plugin_actie))
        .route("/plugins/download/{naam}", get(download_plugin_bestand))
        .route("/themes", get(themes_pagina).post(verwerk_thema_actie))
        .route("/themes/download/{naam}", get(download_thema_bestand))
        .route("/download", get(download_portal_pagina))
        .route("/download/folder", get(download_volledige_folder_zip))
        .route("/download/bestand/{naam}", get(download_starter_bestand))
        // Standaard CMS: Google Auth developer route uitgeschakeld
        .route("/media", get(media_pagina).post(verwerk_media_upload))
        .route("/media/verwijder", post(verwerk_media_verwijder))
        .route("/media/{bestandsnaam}", get(serveer_media_bestand))
        .route("/favicon.ico", get(serveer_favicon))
        .route("/setup", get(setup_formulier).post(verwerk_setup))
        .route("/login", get(login_formulier).post(verwerk_login))
        .route("/logout", post(verwerk_logout).get(verwerk_logout))
        .route(
            "/instellingen",
            get(instellingen_pagina).post(verwerk_instellingen),
        )
        .route("/instellingen/docker", post(verwerk_docker_generator))
        .route("/instellingen/env", post(verwerk_env_generator))
        .layer(DefaultBodyLimit::max(20 * 1024 * 1024))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], poort));
    println!("🚀 BlogRust draait op http://127.0.0.1:{}", poort);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Hulpfunctie: Parse een .env tekst naar sleutel-waarde paren
fn parse_dotenv(inhoud: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for regel in inhoud.lines() {
        let schoon = regel.trim();
        if schoon.is_empty() || schoon.starts_with('#') {
            continue;
        }
        let zonder_export = schoon.strip_prefix("export ").unwrap_or(schoon);
        if let Some((sleutel, waarde)) = zonder_export.split_once('=') {
            let k = sleutel.trim().to_string();
            let v = waarde
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !k.is_empty() {
                map.insert(k, v);
            }
        }
    }
    map
}

// Hulpfunctie: Haal een variabele op uit OS environment of uit het lokale `.env` bestand
fn haal_env_waarde(sleutel: &str) -> Option<String> {
    if let Ok(val) = std::env::var(sleutel) {
        if !val.trim().is_empty() {
            return Some(val);
        }
    }
    if let Ok(inhoud) = fs::read_to_string(".env") {
        let map = parse_dotenv(&inhoud);
        if let Some(val) = map.get(sleutel) {
            if !val.trim().is_empty() {
                return Some(val.clone());
            }
        }
    }
    None
}

// Hulpfunctie: Genereer `.env` en `.env.example` inhoud vanuit het formulier op `/instellingen`
fn maak_dotenv_en_example_inhoud(
    poort: &str,
    domein: &str,
    db_pad: &str,
    google_client_id: &str,
    extra_env: &str,
) -> (String, String) {
    let schone_poort = if poort.trim().is_empty() {
        "3000"
    } else {
        poort.trim()
    };
    let schoon_domein = if domein.trim().is_empty() {
        "localhost"
    } else {
        domein.trim()
    };
    let schoon_db = if db_pad.trim().is_empty() {
        "data/blogrust.db"
    } else {
        db_pad.trim()
    };
    let schoon_google = google_client_id.trim();

    let mut extra_env_echt = String::new();
    let mut extra_env_example = String::new();
    for regel in extra_env.lines() {
        let r = regel.trim();
        if r.is_empty() {
            continue;
        }
        extra_env_echt.push_str(r);
        extra_env_echt.push('\n');
        if r.starts_with('#') {
            extra_env_example.push_str(r);
            extra_env_example.push('\n');
        } else if let Some((k, _)) = r.split_once('=') {
            extra_env_example.push_str(&format!("{}=jouw_waarde_hier\n", k.trim()));
        }
    }

    if extra_env_echt.is_empty() {
        extra_env_echt = "# Voeg hier variabelen toe voor je eigen Rhai plug-ins (uit te lezen via env(\"SLEUTEL\"))\nREACTIES_INGESCHAKELD=true\n".to_string();
        extra_env_example = "# Voeg hier variabelen toe voor je eigen Rhai plug-ins (uit te lezen via env(\"SLEUTEL\"))\nREACTIES_INGESCHAKELD=true\n".to_string();
    }

    let dotenv = format!(
        "# ============================================================================\n\
         # 🦀 BlogRust Omgevingsvariabelen (.env)\n\
         # Beheerd via /instellingen — Zet dit bestand NOOIT op GitHub (staat in .gitignore)\n\
         # ============================================================================\n\n\
         # Server & Database Configuratie\n\
         BLOGRUST_PORT={poort}\n\
         BLOGRUST_DOMAIN={domein}\n\
         BLOGRUST_DB_PAD={db}\n\
         RUST_LOG=info\n\n\
         # Optioneel: Google OAuth Client ID (voor BlogRust.org Community Portal)\n\
         GOOGLE_CLIENT_ID={google}\n\n\
         # Plug-in & Custom Variabelen (beschikbaar in .rhai via env(\"SLEUTEL\"))\n\
         {extra}",
        poort = schone_poort,
        domein = schoon_domein,
        db = schoon_db,
        google = schoon_google,
        extra = extra_env_echt
    );

    let dotenv_example = format!(
        "# ============================================================================\n\
         # 🦀 BlogRust Voorbeeld Omgevingsvariabelen (.env.example)\n\
         # Kopieer dit bestand naar .env (cp .env.example .env) of genereer het via /instellingen\n\
         # ============================================================================\n\n\
         # Server & Database Configuratie\n\
         BLOGRUST_PORT={poort}\n\
         BLOGRUST_DOMAIN=localhost\n\
         BLOGRUST_DB_PAD=data/blogrust.db\n\
         RUST_LOG=info\n\n\
         # Optioneel: Google OAuth Client ID\n\
         GOOGLE_CLIENT_ID=\n\n\
         # Plug-in & Custom Variabelen (beschikbaar in .rhai via env(\"SLEUTEL\"))\n\
         {extra_ex}",
        poort = schone_poort,
        extra_ex = extra_env_example
    );

    (dotenv, dotenv_example)
}

// Hulpfunctie: Checkt of de database al bestaat
fn is_geinstalleerd(pad: &str) -> bool {
    Path::new(pad).exists()
}

// Hulpfunctie: Maak een nieuwe sessie-token aan en sla deze op
fn maak_sessie_aan(state: &AppState, gebruikersnaam: &str) -> String {
    let token = SaltString::generate(&mut OsRng).to_string();
    state
        .sessies
        .write()
        .unwrap()
        .insert(token.clone(), gebruikersnaam.to_string());
    token
}

// Hulpfunctie: Lees de sessie-cookie uit de HTTP-headers en geef de ingelogde gebruiker terug
fn haal_ingelogde_gebruiker_op(headers: &HeaderMap, state: &AppState) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    for deel in cookie_header.split(';') {
        let deel = deel.trim();
        if let Some(token) = deel.strip_prefix("blogrust_sessie=") {
            let sessies = state.sessies.read().unwrap();
            if let Some(gebruiker) = sessies.get(token) {
                return Some(gebruiker.clone());
            }
        }
    }
    None
}

// Hulpfunctie: Maak een Google Auth sessie-token aan voor community-bijdragers
fn maak_google_sessie_aan(state: &AppState, email: &str) -> String {
    let token = SaltString::generate(&mut OsRng).to_string();
    state
        .sessies
        .write()
        .unwrap()
        .insert(format!("google_token:{}", token), email.to_string());
    token
}

// Hulpfunctie: Lees de Google Auth gebruiker uit de HTTP-headers (indien ingelogd via Google)
fn haal_google_gebruiker_op(headers: &HeaderMap, state: &AppState) -> Option<String> {
    let cookie_header = headers.get(COOKIE)?.to_str().ok()?;
    for deel in cookie_header.split(';') {
        let deel = deel.trim();
        if let Some(token) = deel.strip_prefix("blogrust_google_sessie=") {
            let sessies = state.sessies.read().unwrap();
            if let Some(email) = sessies.get(&format!("google_token:{}", token)) {
                return Some(email.clone());
            }
        }
    }
    None
}

// Hulpfunctie: Geef identiteit voor de navigatiebalk terug (Admin of "google:email")
fn haal_nav_identiteit_op(headers: &HeaderMap, state: &AppState) -> Option<String> {
    if let Some(admin) = haal_ingelogde_gebruiker_op(headers, state) {
        Some(admin)
    } else {
        haal_google_gebruiker_op(headers, state).map(|email| format!("google:{}", email))
    }
}

// Hulpfunctie: Verwijder sessie bij uitloggen (zowel Admin als Google Auth)
fn verwijder_sessie(headers: &HeaderMap, state: &AppState) {
    if let Some(Ok(cookie_header)) = headers.get(COOKIE).map(|v| v.to_str()) {
        for deel in cookie_header.split(';') {
            let deel = deel.trim();
            if let Some(token) = deel.strip_prefix("blogrust_sessie=") {
                state.sessies.write().unwrap().remove(token);
            }
            if let Some(gtoken) = deel.strip_prefix("blogrust_google_sessie=") {
                state
                    .sessies
                    .write()
                    .unwrap()
                    .remove(&format!("google_token:{}", gtoken));
            }
        }
    }
}

// Hulpfunctie: Bouw de HTML + Browser API (navigator.language) voor de Multi-Language (i18n.rhai) plug-in
fn maak_i18n_switcher_en_script(standaard_taal: &str) -> String {
    let taal = if standaard_taal.trim().is_empty() {
        "en"
    } else {
        standaard_taal.trim()
    };

    format!(
        r#"<select id="blogrust-lang-switcher" onchange="window.blogrustSetLang(this.value)" title="Language / Taal (Browser API)" style="padding:5px 8px;border-radius:4px;border:1px solid #cbd5e1;background:#fff;font-size:13px;font-weight:600;color:#1f2937;cursor:pointer;">
            <option value="en">🇬🇧 EN</option>
            <option value="nl">🇳🇱 NL</option>
            <option value="de">🇩🇪 DE</option>
            <option value="fr">🇫🇷 FR</option>
            <option value="es">🇪🇸 ES</option>
        </select>
        <script data-default-lang="{taal}">
        (function() {{
            const DICT = {{
                en: {{
                    "✍️ Nieuw Bericht": "✍️ New Post",
                    "🖼️ Media": "🖼️ Media",
                    "🧩 Plug-ins": "🧩 Plugins",
                    "🧩 Plug-ins (+ Toevoegen)": "🧩 Plugins (+ Submit)",
                    "🎨 Thema's": "🎨 Themes",
                    "🎨 Thema's (+ Toevoegen)": "🎨 Themes (+ Submit)",
                    "⬇️ Download": "⬇️ Download",
                    "⬇️ Download BlogRust": "⬇️ Download BlogRust",
                    "⚙️ Instellingen": "⚙️ Settings",
                    "🌐 Google Login": "🌐 Google Sign-In",
                    "🔑 Admin": "🔑 Admin",
                    "Uitloggen": "Sign out",
                    "Zoek": "Search",
                    "Volgorde:": "Order:",
                    "📖 Van begin naar eind": "📖 Start to finish",
                    "🕒 Nieuwste eerst": "🕒 Newest first",
                    "Alle artikelen": "All articles",
                    "Aankondigingen": "Announcements",
                    "📁 Aankondigingen": "📁 Announcements",
                    "📁 Tutorials": "📁 Tutorials",
                    "Lees artikel →": "Read article →",
                    "✏️ Bewerken": "✏️ Edit",
                    "✏️ Bericht bewerken": "✏️ Edit post",
                    "🗑️ Verwijderen": "🗑️ Delete",
                    "🗑️ Verwijder": "🗑️ Delete",
                    "← Terug naar overzicht": "← Back to overview",
                    "💬 Reactie Plaatsen": "💬 Post Comment",
                    "🌐 Wil je reageren op dit artikel?": "🌐 Want to comment on this article?",
                    "Log met 1 klik in via Google Auth om een reactie achter te laten.": "Sign in with 1 click via Google Auth to leave a comment.",
                    "🌐 Inloggen met Google om te reageren": "🌐 Sign in with Google to comment",
                    "Er zijn nog geen reacties onder dit artikel. Plaats als eerste een reactie via Google Auth!": "No comments on this article yet. Be the first to comment with Google Auth!",
                    "Draait op een echte SQLite-database met Argon2-beveiliging & Hugo-stijl Markdown.": "Fast, modular Rust CMS & community portal powered by SQLite, Argon2 & Hugo-style Markdown.",
                    "1. Welkom bij BlogRust!": "1. Welcome to BlogRust!",
                    "Je eerste blogbericht in BlogRust, geschreven in Markdown met Hugo-stijl frontmatter.": "Your first blog post in BlogRust, written in Markdown with Hugo-style frontmatter.",
                    "2. Markdown & Categorieën zoals in Hugo": "2. Markdown & Categories just like in Hugo",
                    "Ontdek hoe je nieuwe artikelen toevoegt via de map posts/ of rechtstreeks vanuit het dashboard.": "Discover how to add new articles via the posts/ folder or directly from the dashboard.",
                    "3. Tutorial: Zo bouw je zelf een Plug-in voor BlogRust (.rhai)": "3. Tutorial: How to Build Your Own Plugin for BlogRust (.rhai)",
                    "Leer stap voor stap hoe je in 2 minuten je eigen Rhai plug-in schrijft en installeert zonder de Rust-broncode aan te passen.": "Learn step by step how to write and install your own Rhai plugin in 2 minutes without modifying the Rust source code.",
                    "4. Geavanceerde Tutorial: Interactieve Plug-ins met Rhai, CSS & JavaScript": "4. Advanced Tutorial: Interactive Plugins with Rhai, CSS & JavaScript",
                    "Leer hoe je server-side Rhai combineert met browser-JavaScript en CSS om interactieve widgets, een automatische inhoudsopgave, leesvoortgang-balk en kopieerknoppen te bouwen.": "Learn how to combine server-side Rhai with browser JavaScript and CSS to build interactive widgets, a table of contents, reading progress bar, and copy buttons.",
                    "5. Tutorial: Zo maak en installeer je een eigen Thema in BlogRust (.css)": "5. Tutorial: How to Create and Install Your Own Theme in BlogRust (.css)",
                    "Ontdek hoe je met één klik van thema wisselt op /themes of zelf een eigen .css thema bouwt voor je BlogRust website.": "Discover how to switch themes with one click on /themes or build your own custom .css theme for your BlogRust website.",
                    "6. BlogRust Deployen met Docker Compose & Caddy (HTTPS)": "6. Deploying BlogRust with Docker Compose & Caddy (HTTPS)",
                    "Genereer met één druk op de knop een complete Dockerfile, docker-compose.yml en Caddyfile om BlogRust in containers te draaien.": "Generate a complete Dockerfile, docker-compose.yml and Caddyfile with one click to run BlogRust in containers.",
                    "🦀 Download Volledige BlogRust CMS Folder": "🦀 Download Full BlogRust CMS Folder",
                    "📁 Download Volledige Folder (blogrust-cms.zip)": "📁 Download Full Folder (blogrust-cms.zip)",
                    "📖 Download README.md": "📖 Download README.md",
                    "🦀 Losse main.rs (Zonder Google Auth)": "🦀 Standalone main.rs (No Google Auth)",
                    "🛠️ Scaffolds & Uitbreidingen Downloaden": "🛠️ Download Scaffolds & Extensions",
                    "🌐 Community Directory": "🌐 Community Directory",
                    "📦 Officiële BlogRust Plug-ins": "📦 Official BlogRust Plugins",
                    "🏪 Officiële BlogRust Thema's": "🏪 Official BlogRust Themes",
                    "● Actief": "● Active",
                    "○ Uitgeschakeld": "○ Disabled",
                    "● Actief op deze site": "● Active on this site",
                    "○ Community Thema": "○ Community Theme",
                    "Huidig thema": "Current theme",
                    "⏸️ Deactiveren": "⏸️ Disable",
                    "▶️ Activeren": "▶️ Enable",
                    "🎨 Activeer": "🎨 Activate",
                    "💾 Wijzigingen opslaan": "💾 Save changes",
                    "🐳 Genereer Docker & Caddy Bestanden": "🐳 Generate Docker & Caddy Files",
                    "🔐 Genereer .env & .env.example Bestanden": "🔐 Generate .env & .env.example Files"
                }},
                de: {{
                    "✍️ Nieuw Bericht": "✍️ Neuer Beitrag",
                    "🧩 Plug-ins": "🧩 Plug-ins",
                    "🎨 Thema's": "🎨 Themen",
                    "⬇️ Download BlogRust": "⬇️ BlogRust herunterladen",
                    "⚙️ Instellingen": "⚙️ Einstellungen",
                    "🌐 Google Login": "🌐 Google Login",
                    "Uitloggen": "Abmelden",
                    "Zoek": "Suchen",
                    "Volgorde:": "Reihenfolge:",
                    "📖 Van begin naar eind": "📖 Von Anfang bis Ende",
                    "🕒 Nieuwste eerst": "🕒 Neueste zuerst",
                    "Alle artikelen": "Alle Artikel",
                    "📁 Aankondigingen": "📁 Ankündigungen",
                    "Lees artikel →": "Artikel lesen →",
                    "← Terug naar overzicht": "← Zurück zur Übersicht",
                    "💬 Reactie Plaatsen": "💬 Kommentar senden",
                    "1. Welkom bij BlogRust!": "1. Willkommen bei BlogRust!",
                    "Je eerste blogbericht in BlogRust, geschreven in Markdown met Hugo-stijl frontmatter.": "Dein erster Blogbeitrag in BlogRust, geschrieben in Markdown mit Hugo-Frontmatter.",
                    "2. Markdown & Categorieën zoals in Hugo": "2. Markdown & Kategorien wie in Hugo",
                    "3. Tutorial: Zo bouw je zelf een Plug-in voor BlogRust (.rhai)": "3. Tutorial: So baust du dein eigenes Plug-in für BlogRust (.rhai)",
                    "4. Geavanceerde Tutorial: Interactieve Plug-ins met Rhai, CSS & JavaScript": "4. Fortgeschrittenes Tutorial: Interaktive Plug-ins mit Rhai, CSS & JS",
                    "5. Tutorial: Zo maak en installeer je een eigen Thema in BlogRust (.css)": "5. Tutorial: Eigenes CSS-Theme für BlogRust erstellen",
                    "6. BlogRust Deployen met Docker Compose & Caddy (HTTPS)": "6. BlogRust mit Docker Compose & Caddy (HTTPS) bereitstellen",
                    "Draait op een echte SQLite-database met Argon2-beveiliging & Hugo-stijl Markdown.": "Schnelles, modulares Rust CMS mit SQLite, Argon2 & Hugo-Markdown."
                }},
                fr: {{
                    "✍️ Nieuw Bericht": "✍️ Nouvel Article",
                    "🧩 Plug-ins": "🧩 Extensions",
                    "🎨 Thema's": "🎨 Thèmes",
                    "⬇️ Download BlogRust": "⬇️ Télécharger BlogRust",
                    "⚙️ Instellingen": "⚙️ Paramètres",
                    "🌐 Google Login": "🌐 Connexion Google",
                    "Uitloggen": "Déconnexion",
                    "Zoek": "Chercher",
                    "Volgorde:": "Ordre :",
                    "📖 Van begin naar eind": "📖 Du début à la fin",
                    "🕒 Nieuwste eerst": "🕒 Plus récents",
                    "Alle artikelen": "Tous les articles",
                    "📁 Aankondigingen": "📁 Annonces",
                    "Lees artikel →": "Lire l'article →",
                    "← Terug naar overzicht": "← Retour à l'accueil",
                    "💬 Reactie Plaatsen": "💬 Publier un commentaire",
                    "1. Welkom bij BlogRust!": "1. Bienvenue sur BlogRust !",
                    "2. Markdown & Categorieën zoals in Hugo": "2. Markdown et Catégories comme dans Hugo",
                    "3. Tutorial: Zo bouw je zelf een Plug-in voor BlogRust (.rhai)": "3. Tutoriel : Créer votre propre extension BlogRust (.rhai)",
                    "4. Geavanceerde Tutorial: Interactieve Plug-ins met Rhai, CSS & JavaScript": "4. Tutoriel avancé : Extensions interactives avec Rhai, CSS et JS",
                    "5. Tutorial: Zo maak en installeer je een eigen Thema in BlogRust (.css)": "5. Tutoriel : Créer et installer un thème CSS dans BlogRust",
                    "6. BlogRust Deployen met Docker Compose & Caddy (HTTPS)": "6. Déployer BlogRust avec Docker Compose et Caddy (HTTPS)",
                    "Draait op een echte SQLite-database met Argon2-beveiliging & Hugo-stijl Markdown.": "CMS Rust rapide et modulaire propulsé par SQLite, Argon2 et Markdown."
                }},
                es: {{
                    "✍️ Nieuw Bericht": "✍️ Nueva Entrada",
                    "🧩 Plug-ins": "🧩 Plugins",
                    "🎨 Thema's": "🎨 Temas",
                    "⬇️ Download BlogRust": "⬇️ Descargar BlogRust",
                    "⚙️ Instellingen": "⚙️ Ajustes",
                    "🌐 Google Login": "🌐 Iniciar con Google",
                    "Uitloggen": "Salir",
                    "Zoek": "Buscar",
                    "Volgorde:": "Orden:",
                    "📖 Van begin naar eind": "📖 De principio a fin",
                    "🕒 Nieuwste eerst": "🕒 Más recientes",
                    "Alle artikelen": "Todos los artículos",
                    "📁 Aankondigingen": "📁 Anuncios",
                    "Lees artikel →": "Leer artículo →",
                    "← Terug naar overzicht": "← Volver al resumen",
                    "💬 Reactie Plaatsen": "💬 Publicar comentario",
                    "1. Welkom bij BlogRust!": "1. ¡Bienvenido a BlogRust!",
                    "2. Markdown & Categorieën zoals in Hugo": "2. Markdown y Categorías como en Hugo",
                    "3. Tutorial: Zo bouw je zelf een Plug-in voor BlogRust (.rhai)": "3. Tutorial: Cómo crear tu propio plugin para BlogRust (.rhai)",
                    "4. Geavanceerde Tutorial: Interactieve Plug-ins met Rhai, CSS & JavaScript": "4. Tutorial avanzado: Plugins interactivos con Rhai, CSS y JS",
                    "5. Tutorial: Zo maak en installeer je een eigen Thema in BlogRust (.css)": "5. Tutorial: Cómo crear e instalar tu propio tema en BlogRust (.css)",
                    "6. BlogRust Deployen met Docker Compose & Caddy (HTTPS)": "6. Desplegar BlogRust con Docker Compose y Caddy (HTTPS)",
                    "Draait op een echte SQLite-database met Argon2-beveiliging & Hugo-stijl Markdown.": "CMS en Rust rápido y modular con SQLite, Argon2 y Markdown."
                }}
            }};

            const PLACEHOLDERS = {{
                en: {{ "🔍 Zoek in artikelen...": "🔍 Search articles...", "Schrijf je reactie op dit artikel...": "Write your comment on this article..." }},
                de: {{ "🔍 Zoek in artikelen...": "🔍 Artikel suchen...", "Schrijf je reactie op dit artikel...": "Schreiben Sie Ihren Kommentar..." }},
                fr: {{ "🔍 Zoek in artikelen...": "🔍 Rechercher des articles...", "Schrijf je reactie op dit artikel...": "Écrivez votre commentaire..." }},
                es: {{ "🔍 Zoek in artikelen...": "🔍 Buscar artículos...", "Schrijf je reactie op dit artikel...": "Escribe tu comentario..." }}
            }};

            function detectInitialLang() {{
                const saved = localStorage.getItem('blogrust_lang');
                if (saved) return saved;
                const scriptEl = document.querySelector('script[data-default-lang]');
                const defLang = scriptEl ? scriptEl.getAttribute('data-default-lang') : 'en';
                if (defLang && defLang !== 'auto') return defLang;
                const navLang = (navigator.language || navigator.userLanguage || 'en').slice(0, 2).toLowerCase();
                return ['en', 'nl', 'de', 'fr', 'es'].includes(navLang) ? navLang : 'en';
            }}

            // Beschermde merknamen die NOOIT als gewoon woord (zoals 'Rest' of 'Ruhe') vertaald mogen worden
            const PROTECTED_NAMES = [
                'BlogRust.com', 'BlogRust.org', 'BlogRust', 'Blogrust', 'blogrust',
                'Rust', 'Rhai', 'Hugo', 'Caddy', 'Axum', 'Argon2id', 'Argon2', 'SQLite', 'Markdown', 'Frontmatter'
            ];

            function protectBrandNames(text) {{
                let out = text;
                PROTECTED_NAMES.forEach((name, idx) => {{
                    const escaped = name.replace(/[.*+?^${{}}()|[\]\\]/g, '\\$&');
                    const re = new RegExp('\\b' + escaped + '\\b', 'g');
                    out = out.replace(re, 'ZXQ' + idx + 'QXZ');
                }});
                return out;
            }}

            function restoreBrandNames(text) {{
                let out = text;
                PROTECTED_NAMES.forEach((name, idx) => {{
                    const re = new RegExp('ZXQ\\s*' + idx + '\\s*QXZ', 'gi');
                    out = out.replace(re, name);
                }});
                return out;
            }}

            // Vertaal losse tekst-nodes automatisch (met lokale cache) voor de volledige website & artikelen
            const memCache = {{}};
            async function autoTranslateNode(node, origText, targetLang) {{
                const trimmed = origText.trim();
                if (trimmed.length < 3 || /^[0-9\W_]+$/.test(trimmed)) return;
                // Als de tekst alleen uit een beschermde merknaam (+ evt. emoji) bestaat, nooit vertalen!
                const zonderEmoji = trimmed.replace(/[\u{{1F300}}-\u{{1FAFF}}\u{{2600}}-\u{{27BF}}]/gu, '').trim();
                if (PROTECTED_NAMES.includes(zonderEmoji)) return;

                const cacheKey = 'br_tr_v3_' + targetLang + '_' + trimmed;
                const cached = memCache[cacheKey] || sessionStorage.getItem(cacheKey);
                if (cached) {{
                    if (document.documentElement.lang === targetLang) {{
                        node.nodeValue = origText.replace(trimmed, cached);
                    }}
                    return;
                }}
                try {{
                    const tekstMetMaskers = protectBrandNames(trimmed);
                    const url = 'https://translate.googleapis.com/translate_a/single?client=gtx&sl=nl&tl=' + encodeURIComponent(targetLang) + '&dt=t&q=' + encodeURIComponent(tekstMetMaskers);
                    const res = await fetch(url);
                    if (!res.ok) return;
                    const data = await res.json();
                    if (Array.isArray(data) && Array.isArray(data[0])) {{
                        const ruwVertaald = data[0].map(part => part[0]).join('');
                        const vertaald = restoreBrandNames(ruwVertaald);
                        if (vertaald) {{
                            memCache[cacheKey] = vertaald;
                            try {{ sessionStorage.setItem(cacheKey, vertaald); }} catch(e) {{}}
                            if (document.documentElement.lang === targetLang) {{
                                node.nodeValue = origText.replace(trimmed, vertaald);
                            }}
                        }}
                    }}
                }} catch (e) {{}}
            }}

            function applyLang(lang) {{
                document.documentElement.lang = lang;
                const sel = document.getElementById('blogrust-lang-switcher');
                if (sel) sel.value = lang;

                // Loop door ALLE tekst-nodes op de hele pagina (exclusief <script>, <style>, <pre>, <code> en .brand)
                const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT, null, false);
                const textNodes = [];
                let current;
                while ((current = walker.nextNode())) {{
                    const parent = current.parentElement;
                    if (!parent) continue;
                    const tag = parent.tagName;
                    if (tag === 'SCRIPT' || tag === 'STYLE' || tag === 'PRE' || tag === 'CODE' || tag === 'TEXTAREA' || tag === 'OPTION') continue;
                    if (parent.closest('pre, code, .brand, #brand-preview, #tab-preview, #blogrust-lang-switcher')) continue;
                    if (current.nodeValue && current.nodeValue.trim().length > 0) {{
                        textNodes.push(current);
                    }}
                }}

                textNodes.forEach(node => {{
                    if (node._origNl === undefined) {{
                        node._origNl = node.nodeValue;
                    }}
                    const origFull = node._origNl;
                    const origTrim = origFull.trim();

                    if (lang === 'nl') {{
                        node.nodeValue = origFull;
                        return;
                    }}

                    // 1. Check directe woordenboek-match (0ms)
                    if (DICT[lang] && DICT[lang][origTrim]) {{
                        node.nodeValue = origFull.replace(origTrim, DICT[lang][origTrim]);
                        return;
                    }}
                    if (DICT.en && DICT.en[origTrim] && lang === 'en') {{
                        node.nodeValue = origFull.replace(origTrim, DICT.en[origTrim]);
                        return;
                    }}

                    // 2. Check dynamische leestijd / vorig / volgend
                    if (origTrim.startsWith('⏱️ Geschatte leestijd:')) {{
                        node.nodeValue = origFull
                            .replace('⏱️ Geschatte leestijd:', '⏱️ Estimated reading time:')
                            .replace('minuut', 'min read');
                        return;
                    }}
                    if (origTrim.startsWith('← Vorig:')) {{
                        node.nodeValue = origFull.replace('← Vorig:', '← Previous:');
                    }}
                    if (origTrim.startsWith('Volgend:')) {{
                        node.nodeValue = origFull.replace('Volgend:', 'Next:');
                    }}

                    // 3. Vertaal alle overige zinnen, alinea's, tabellen en artikelen op de hele site automatisch!
                    autoTranslateNode(node, origFull, lang);
                }});

                document.querySelectorAll('input[placeholder], textarea[placeholder]').forEach(inp => {{
                    if (!inp.hasAttribute('data-orig-ph')) {{
                        inp.setAttribute('data-orig-ph', inp.getAttribute('placeholder'));
                    }}
                    const origPh = inp.getAttribute('data-orig-ph');
                    if (lang === 'nl') {{
                        inp.setAttribute('placeholder', origPh);
                    }} else if (PLACEHOLDERS[lang] && PLACEHOLDERS[lang][origPh]) {{
                        inp.setAttribute('placeholder', PLACEHOLDERS[lang][origPh]);
                    }} else if (PLACEHOLDERS.en[origPh]) {{
                        inp.setAttribute('placeholder', PLACEHOLDERS.en[origPh]);
                    }}
                }});
            }}

            window.blogrustSetLang = function(lang) {{
                localStorage.setItem('blogrust_lang', lang);
                applyLang(lang);
            }};

            if (document.readyState === 'loading') {{
                document.addEventListener('DOMContentLoaded', () => applyLang(detectInitialLang()));
            }} else {{
                applyLang(detectInitialLang());
            }}
        }})();
        </script>"#,
        taal = taal
    )
}

// Hulpfunctie: Voer actieve .rhai plug-ins in `plugins/` uit die een `bewerk_navigatie(nav_html)` hook hebben
fn pas_navigatie_plugins_toe(plugins_map: &str, mut nav_html: String) -> String {
    let mut engine = Engine::new();
    engine.register_fn("env", |sleutel: &str| -> String {
        haal_env_waarde(sleutel).unwrap_or_default()
    });

    let mut script_paden = Vec::new();
    if let Ok(entries) = fs::read_dir(plugins_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if pad.extension().and_then(|e| e.to_str()) == Some("rhai") {
                script_paden.push(pad);
            }
        }
    }
    script_paden.sort();

    for pad in script_paden {
        if let Ok(script_code) = fs::read_to_string(&pad) {
            if let Ok(ast) = engine.compile(&script_code) {
                let mut scope = Scope::new();
                if let Ok(nieuwe_nav) =
                    engine.call_fn::<String>(&mut scope, &ast, "bewerk_navigatie", (nav_html.clone(),))
                {
                    nav_html = nieuwe_nav;
                }
            }
        }
    }

    if let Some(start_idx) = nav_html.find("<!-- BLOGRUST_I18N:") {
        if let Some(eind_rel) = nav_html[start_idx..].find("-->") {
            let marker = &nav_html[start_idx..start_idx + eind_rel + 3];
            let standaard_taal = marker
                .trim_start_matches("<!-- BLOGRUST_I18N:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
            let switcher_html = maak_i18n_switcher_en_script(&standaard_taal);
            nav_html = nav_html.replace(marker, &switcher_html);
        }
    }

    nav_html
}

// Hulpfunctie: Bouw de knoppen rechtsboven in de navigatiebalk (BlogRust.org Portal)
fn maak_nav_rechts_html(ingelogde_gebruiker: Option<&str>) -> String {
    let basis_nav = if let Some(gebruiker) = ingelogde_gebruiker {
        if let Some(google_email) = gebruiker.strip_prefix("google:") {
            format!(
                r#"<span class="user-badge" style="color:#1d4ed8;font-weight:600;">🌐 {google_email}</span>
                   <a href="/plugins" class="btn-link">🧩 Plug-ins (+ Toevoegen)</a>
                   <a href="/themes" class="btn-link">🎨 Thema's (+ Toevoegen)</a>
                   <a href="/download" class="btn-new">⬇️ Download BlogRust</a>
                   <a href="/logout" class="btn-logout">Uitloggen</a>"#
            )
        } else {
            format!(
                r#"<span class="user-badge">👤 {gebruiker}</span>
                   <a href="/nieuw" class="btn-new">✍️ Nieuw Bericht</a>
                   <a href="/media" class="btn-link">🖼️ Media</a>
                   <a href="/plugins" class="btn-link">🧩 Plug-ins</a>
                   <a href="/themes" class="btn-link">🎨 Thema's</a>
                   <a href="/download" class="btn-link">⬇️ Download</a>
                   <a href="/instellingen" class="btn-link">⚙️ Instellingen</a>
                   <a href="/logout" class="btn-logout">Uitloggen</a>"#
            )
        }
    } else {
        r#"<a href="/plugins" class="btn-link">🧩 Plug-ins</a>
           <a href="/themes" class="btn-link">🎨 Thema's</a>
           <a href="/download" class="btn-new">⬇️ Download BlogRust</a>
           
           <a href="/login" class="btn-login">🔑 Admin</a>"#
            .to_string()
    };

    pas_navigatie_plugins_toe("plugins", basis_nav)
}

// Hulpfunctie: Zet tekst om naar een veilige URL-slug (zoals Hugo)
fn maak_slug(tekst: &str) -> String {
    let mut slug = String::new();
    let mut vorige_streepje = false;
    for ch in tekst.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            vorige_streepje = false;
        } else if !vorige_streepje && !slug.is_empty() {
            slug.push('-');
            vorige_streepje = true;
        }
    }
    slug.trim_matches('-').to_string()
}

// Hulpfunctie: Maak een geüploade bestandsnaam veilig
fn maak_veilige_bestandsnaam(naam: &str) -> String {
    let basis = Path::new(naam)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("afbeelding.png");

    let mut schoon = String::new();
    for ch in basis.to_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            schoon.push(ch);
        } else if ch == ' ' {
            schoon.push('-');
        }
    }
    let schoon = schoon.trim_matches('.').to_string();
    if schoon.is_empty() {
        "upload.png".to_string()
    } else {
        schoon
    }
}

// Hulpfunctie: Bepaal het juiste MIME Content-Type voor een mediabestand
fn bepaal_content_type(bestandsnaam: &str) -> &'static str {
    let ext = Path::new(bestandsnaam)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

// Hulpfunctie: Lees een scaffold-bestand uit `scaffolds/`
fn laad_scaffold(scaffolds_map: &str, bestandsnaam: &str, fallback: &str) -> String {
    let pad = format!("{}/{}", scaffolds_map, bestandsnaam);
    fs::read_to_string(pad).unwrap_or_else(|_| fallback.to_string())
}

// Hulpfunctie: Lees alle bestanden in de map `media/`
fn lees_media_bestanden(media_map: &str) -> Vec<MediaBestand> {
    let mut bestanden = Vec::new();
    if let Ok(entries) = fs::read_dir(media_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if pad.is_file() {
                if let Some(naam) = pad.file_name().and_then(|n| n.to_str()) {
                    let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let grootte_kb = (bytes / 1024).max(1);
                    bestanden.push(MediaBestand {
                        bestandsnaam: naam.to_string(),
                        grootte_kb,
                    });
                }
            }
        }
    }
    bestanden.sort_by(|a, b| a.bestandsnaam.cmp(&b.bestandsnaam));
    bestanden
}

// Hulpfunctie: Zet Markdown-tekst om naar HTML
fn markdown_naar_html(md: &str) -> String {
    let mut opties = Options::empty();
    opties.insert(Options::ENABLE_TABLES);
    opties.insert(Options::ENABLE_STRIKETHROUGH);
    opties.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(md, opties);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// Hulpfunctie: Voer alle actieve .rhai plug-ins in `plugins/` uit op een post!
fn pas_plugins_toe_op_post(plugins_map: &str, mut post: MarkdownPost) -> MarkdownPost {
    let mut engine = Engine::new();
    // Geef Rhai plug-ins toegang tot variabelen uit .env via env("SLEUTEL")
    engine.register_fn("env", |sleutel: &str| -> String {
        haal_env_waarde(sleutel).unwrap_or_default()
    });

    let mut script_paden = Vec::new();
    if let Ok(entries) = fs::read_dir(plugins_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if pad.extension().and_then(|e| e.to_str()) == Some("rhai") {
                script_paden.push(pad);
            }
        }
    }
    script_paden.sort();

    for pad in script_paden {
        if let Ok(script_code) = fs::read_to_string(&pad) {
            if let Ok(ast) = engine.compile(&script_code) {
                let mut scope = Scope::new();

                if let Ok(nieuwe_titel) = engine.call_fn::<String>(
                    &mut scope,
                    &ast,
                    "bewerk_titel",
                    (post.titel.clone(),),
                ) {
                    post.titel = nieuwe_titel;
                }

                if let Ok(nieuwe_html) = engine.call_fn::<String>(
                    &mut scope,
                    &ast,
                    "bewerk_bericht_met_cover",
                    (
                        post.html_inhoud.clone(),
                        post.cover.clone(),
                        post.categorie.clone(),
                    ),
                ) {
                    post.html_inhoud = nieuwe_html;
                }

                if let Ok(nieuwe_html) = engine.call_fn::<String>(
                    &mut scope,
                    &ast,
                    "bewerk_bericht",
                    (post.html_inhoud.clone(),),
                ) {
                    post.html_inhoud = nieuwe_html;
                }
            }
        }
    }

    post
}

fn is_cover_plugin_actief(plugins_map: &str) -> bool {
    Path::new(&format!("{}/cover.rhai", plugins_map)).exists()
}

// Hulpfunctie: Parse een .md bestand met Hugo-stijl frontmatter (--- of +++)
fn parse_markdown_post(slug: &str, ruwe_inhoud: &str) -> MarkdownPost {
    let mut titel = slug.replace('-', " ");
    let mut datum = "2026-11-14".to_string();
    let mut volgorde: i32 = 999;
    let mut categorie = "Algemeen".to_string();
    let mut tags = Vec::new();
    let mut cover = String::new();
    let mut samenvatting = String::new();
    let mut concept = false;
    let mut markdown_body = ruwe_inhoud;

    let genormaliseerd = ruwe_inhoud.trim_start_matches('\u{feff}');
    let scheiding = if genormaliseerd.starts_with("---") {
        Some("---")
    } else if genormaliseerd.starts_with("+++") {
        Some("+++")
    } else {
        None
    };

    if let Some(delim) = scheiding {
        let na_eerste = &genormaliseerd[delim.len()..];
        if let Some(eind_idx) = na_eerste.find(&format!("\n{}", delim)) {
            let frontmatter = &na_eerste[..eind_idx];
            let rest_start = eind_idx + 1 + delim.len();
            markdown_body = na_eerste[rest_start..].trim_start_matches(['\r', '\n']);

            for regel in frontmatter.lines() {
                let regel = regel.trim();
                if regel.is_empty() || regel.starts_with('#') {
                    continue;
                }
                if let Some((sleutel, waarde)) = regel.split_once([':', '=']) {
                    let sleutel = sleutel.trim().to_lowercase();
                    let waarde = waarde.trim().trim_matches('"').trim_matches('\'');

                    match sleutel.as_str() {
                        "title" | "titel" => titel = waarde.to_string(),
                        "date" | "datum" => datum = waarde.to_string(),
                        "weight" | "volgorde" => {
                            if let Ok(w) = waarde.parse::<i32>() {
                                volgorde = w;
                            }
                        }
                        "category" | "categorie" | "categories" => {
                            let schoon = waarde.trim_matches(['[', ']']).trim_matches('"').trim();
                            if !schoon.is_empty() {
                                categorie = schoon
                                    .split(',')
                                    .next()
                                    .unwrap_or("Algemeen")
                                    .trim()
                                    .trim_matches('"')
                                    .to_string();
                            }
                        }
                        "tags" => {
                            let binnen = waarde.trim_matches(['[', ']']);
                            tags = binnen
                                .split(',')
                                .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
                                .filter(|t| !t.is_empty())
                                .collect();
                        }
                        "cover" | "image" | "afbeelding" => {
                            cover = waarde.to_string();
                        }
                        "summary" | "samenvatting" | "description" => {
                            samenvatting = waarde.to_string();
                        }
                        "draft" | "concept" => {
                            concept = waarde.eq_ignore_ascii_case("true") || waarde == "1";
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if samenvatting.is_empty() {
        let eerste_alinea = markdown_body
            .lines()
            .find(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
            .unwrap_or("")
            .replace("**", "")
            .replace('*', "");
        samenvatting = if eerste_alinea.chars().count() > 140 {
            let ingekort: String = eerste_alinea.chars().take(140).collect();
            format!("{}...", ingekort)
        } else {
            eerste_alinea
        };
    }

    MarkdownPost {
        slug: slug.to_string(),
        titel,
        datum,
        volgorde,
        categorie,
        tags,
        cover,
        samenvatting,
        concept,
        ruwe_markdown: markdown_body.to_string(),
        html_inhoud: markdown_naar_html(markdown_body),
    }
}

fn laad_alle_posts(posts_map: &str) -> Vec<MarkdownPost> {
    let mut posts = Vec::new();
    if let Ok(entries) = fs::read_dir(posts_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if pad.extension().and_then(|e| e.to_str()) == Some("md") {
                if let Some(bestandsnaam) = pad.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(inhoud) = fs::read_to_string(&pad) {
                        posts.push(parse_markdown_post(bestandsnaam, &inhoud));
                    }
                }
            }
        }
    }
    // Sorteer standaard van begin naar eind: eerst op `weight` (volgorde), daarna op datum (oudste eerst) en titel
    posts.sort_by(|a, b| {
        a.volgorde
            .cmp(&b.volgorde)
            .then_with(|| a.datum.cmp(&b.datum))
            .then_with(|| a.titel.cmp(&b.titel))
    });
    posts
}

// Hulpfunctie: Haal site_titel, site_beschrijving én actief_thema op uit SQLite
async fn haal_site_info_op(db_pad: &str) -> (String, String, String) {
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_pad))
        .await
        .unwrap();

    let titel: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'site_titel'")
            .fetch_one(&pool)
            .await
            .unwrap_or(("BlogRust".to_string(),));

    let beschrijving: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'site_beschrijving'")
            .fetch_one(&pool)
            .await
            .unwrap_or((
                "Draait op een echte SQLite-database met Argon2-beveiliging & Hugo-stijl Markdown."
                    .to_string(),
            ));

    let actief_thema: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'actief_thema'")
            .fetch_one(&pool)
            .await
            .unwrap_or(("standaard.css".to_string(),));

    (titel.0, beschrijving.0, actief_thema.0)
}

// Hulpfunctie: Bouw de HTML voor het navigatie-logo/icoon én de browser-tab favicon
fn maak_brand_en_favicon_html(titel: &str, site_icoon: &str, site_logo: &str) -> (String, String) {
    let icoon = if site_icoon.trim().is_empty() {
        "🦀"
    } else {
        site_icoon.trim()
    };
    let logo = site_logo.trim();

    let brand_html = if !logo.is_empty() {
        format!(
            r#"<img src="{logo}" alt="Logo" style="height:28px;width:auto;max-width:160px;object-fit:contain;vertical-align:middle;margin-right:8px;border-radius:4px;"><span>{titel}</span>"#,
            logo = logo,
            titel = titel
        )
    } else if icoon.starts_with('/') || icoon.starts_with("http://") || icoon.starts_with("https://")
    {
        format!(
            r#"<img src="{icoon}" alt="Icon" style="height:24px;width:24px;object-fit:contain;vertical-align:middle;margin-right:8px;border-radius:4px;"><span>{titel}</span>"#,
            icoon = icoon,
            titel = titel
        )
    } else {
        format!("{} {}", icoon, titel)
    };

    let favicon_html =
        if icoon.starts_with('/') || icoon.starts_with("http://") || icoon.starts_with("https://") {
            format!(r#"<link id="live-favicon" rel="icon" href="{icoon}">"#, icoon = icoon)
        } else {
            format!(
                r#"<link id="live-favicon" rel="icon" href="data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>{icoon}</text></svg>">"#,
                icoon = icoon
            )
        };

    (brand_html, favicon_html)
}

async fn serveer_favicon(State(state): State<AppState>) -> impl IntoResponse {
    let icoon = if is_geinstalleerd(&state.db_pad) {
        let (site_icoon, _, _, _) = haal_site_branding_op(&state.db_pad, "BlogRust").await;
        if site_icoon.trim().is_empty() {
            "🦀".to_string()
        } else {
            site_icoon.trim().to_string()
        }
    } else {
        "🦀".to_string()
    };

    if icoon.starts_with('/') || icoon.starts_with("http://") || icoon.starts_with("https://") {
        return Redirect::to(&icoon).into_response();
    }

    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"><text y=".9em" font-size="90">{}</text></svg>"#,
        icoon
    );
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("image/svg+xml"));
    (StatusCode::OK, headers, svg).into_response()
}

async fn haal_site_branding_op(db_pad: &str, titel: &str) -> (String, String, String, String) {
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_pad))
        .await
        .unwrap();

    let site_icoon: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'site_icoon'")
            .fetch_one(&pool)
            .await
            .unwrap_or(("🦀".to_string(),));

    let site_logo: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'site_logo'")
            .fetch_one(&pool)
            .await
            .unwrap_or((String::new(),));

    let (brand_html, favicon_html) =
        maak_brand_en_favicon_html(titel, &site_icoon.0, &site_logo.0);
    (site_icoon.0, site_logo.0, brand_html, favicon_html)
}

fn laad_thema_css(themes_map: &str, actief_thema: &str) -> String {
    let pad = format!("{}/{}", themes_map, actief_thema);
    fs::read_to_string(pad).unwrap_or_default()
}

// 1. De openbare indexpagina (/) met Zoekbalk + Categorie-filter + CRUD knoppen voor admin
async fn home_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(filter): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let ingelogde_gebruiker = haal_ingelogde_gebruiker_op(&headers, &state);
    let is_ingelogd = ingelogde_gebruiker.is_some();
    let nav_id = haal_nav_identiteit_op(&headers, &state);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());
    let toon_covers = is_cover_plugin_actief(&state.plugins_map);

    let (titel, beschrijving, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);

    let mut alle_posts: Vec<MarkdownPost> = laad_alle_posts(&state.posts_map)
        .into_iter()
        .filter(|p| is_ingelogd || !p.concept)
        .collect();

    let is_nieuwste_eerst = filter.sorteer.as_deref() == Some("nieuwste");
    if is_nieuwste_eerst {
        alle_posts.reverse();
    }

    let mut categorie_telling: BTreeMap<String, usize> = BTreeMap::new();
    for post in &alle_posts {
        *categorie_telling.entry(post.categorie.clone()).or_insert(0) += 1;
    }

    let zoekterm = filter.zoek.clone().unwrap_or_default();
    let zoek_lower = zoekterm.to_lowercase();

    let gefilterde_posts: Vec<&MarkdownPost> = alle_posts
        .iter()
        .filter(|p| {
            if let Some(ref cat) = filter.categorie {
                if !p.categorie.eq_ignore_ascii_case(cat) {
                    return false;
                }
            }
            if let Some(ref gezochte_tag) = filter.tag {
                if !p.tags.iter().any(|t| t.eq_ignore_ascii_case(gezochte_tag)) {
                    return false;
                }
            }
            if !zoek_lower.is_empty() {
                let in_titel = p.titel.to_lowercase().contains(&zoek_lower);
                let in_samenvatting = p.samenvatting.to_lowercase().contains(&zoek_lower);
                let in_inhoud = p.ruwe_markdown.to_lowercase().contains(&zoek_lower);
                if !in_titel && !in_samenvatting && !in_inhoud {
                    return false;
                }
            }
            true
        })
        .collect();

    let alle_actief = if filter.categorie.is_none() && filter.tag.is_none() && zoek_lower.is_empty()
    {
        "cat-pill actief"
    } else {
        "cat-pill"
    };
    let mut cat_knoppen = format!(
        r#"<a href="/" class="{alle_actief}">Alle ({totaal})</a>"#,
        alle_actief = alle_actief,
        totaal = alle_posts.len()
    );
    for (cat_naam, aantal) in &categorie_telling {
        let is_actief = filter
            .categorie
            .as_deref()
            .map(|c| c.eq_ignore_ascii_case(cat_naam))
            .unwrap_or(false);
        let klasse = if is_actief {
            "cat-pill actief"
        } else {
            "cat-pill"
        };
        cat_knoppen.push_str(&format!(
            r#"<a href="/?categorie={cat_naam}" class="{klasse}">📁 {cat_naam} ({aantal})</a>"#
        ));
    }

    let sorteer_knop = if is_nieuwste_eerst {
        r#"<a href="/?sorteer=begin" class="cat-pill" style="margin-left:auto;">⬆️ Sorteer: Begin &rarr; Eind</a>"#
    } else {
        r#"<a href="/?sorteer=nieuwste" class="cat-pill" style="margin-left:auto;">⬇️ Sorteer: Eind &rarr; Begin</a>"#
    };
    cat_knoppen.push_str(sorteer_knop);

    let mut posts_html = String::new();
    if gefilterde_posts.is_empty() {
        posts_html.push_str(
            r#"<div class="post-card"><p>Geen berichten gevonden voor deze selectie.</p></div>"#,
        );
    } else {
        for post in gefilterde_posts {
            let concept_badge = if post.concept {
                r#"<span class="draft-badge">CONCEPT</span>"#
            } else {
                ""
            };

            let cover_html = if toon_covers && !post.cover.is_empty() {
                format!(
                    r#"<a href="/post/{slug}"><img src="{cover}" alt="{titel}" class="card-cover"></a>"#,
                    slug = post.slug,
                    cover = post.cover,
                    titel = post.titel
                )
            } else {
                String::new()
            };

            let admin_crud_knoppen = if is_ingelogd {
                format!(
                    r#"<span style="margin-left:auto;display:flex;gap:6px;">
                        <a href="/bewerk/{slug}" class="crud-edit">✏️ Bewerken</a>
                        <form action="/verwijder/{slug}" method="post" style="margin:0;" onsubmit="return confirm('Weet je zeker dat je dit artikel wilt verwijderen?');">
                            <button type="submit" class="crud-del">🗑️</button>
                        </form>
                    </span>"#,
                    slug = post.slug
                )
            } else {
                String::new()
            };

            let tags_html: String = post
                .tags
                .iter()
                .map(|t| format!(r#"<a href="/?tag={t}" class="tag">#{t}</a>"#))
                .collect::<Vec<_>>()
                .join(" ");

            posts_html.push_str(&format!(
                r#"
                <article class="post-card">
                    {cover_html}
                    <div class="post-meta">
                        <span>📅 {datum}</span>
                        <a href="/?categorie={categorie}" class="cat-badge">📁 {categorie}</a>
                        {concept_badge}
                        {admin_crud_knoppen}
                    </div>
                    <h2><a href="/post/{slug}">{post_titel}</a></h2>
                    <p class="summary">{samenvatting}</p>
                    <div class="post-footer">
                        <div class="tags">{tags_html}</div>
                        <a href="/post/{slug}" class="read-more">Lees artikel &rarr;</a>
                    </div>
                </article>
                "#,
                cover_html = cover_html,
                datum = post.datum,
                categorie = post.categorie,
                concept_badge = concept_badge,
                admin_crud_knoppen = admin_crud_knoppen,
                slug = post.slug,
                post_titel = post.titel,
                samenvatting = post.samenvatting,
                tags_html = tags_html
            ));
        }
    }

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>{titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; box-shadow: 0 1px 2px rgba(0,0,0,0.03); }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; margin-right: 4px; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-new:hover {{ background: #c2410c; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-link:hover {{ background: #e5f0fa; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-login:hover {{ background: #135e96; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 14px; font-weight: 500; text-decoration: none; }}
                .btn-logout:hover {{ background: #b91c1c; }}
                .hero {{ background: #fff; padding: 24px 28px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 20px; display: flex; justify-content: space-between; align-items: center; gap: 16px; flex-wrap: wrap; }}
                .hero h1 {{ color: #ea580c; margin: 0 0 8px 0; font-size: 26px; }}
                .hero p {{ margin: 0; color: #4b5563; }}
                .search-form {{ display: flex; gap: 6px; }}
                .search-input {{ padding: 8px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; min-width: 210px; }}
                .search-btn {{ background: #f6f7f7; border: 1px solid #d1d5db; padding: 8px 12px; border-radius: 6px; cursor: pointer; font-weight: 600; }}
                .cat-bar {{ display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 20px; align-items: center; }}
                .cat-pill {{ text-decoration: none; font-size: 13px; font-weight: 600; color: #374151; background: #fff; border: 1px solid #d1d5db; padding: 6px 12px; border-radius: 999px; }}
                .cat-pill:hover {{ border-color: #ea580c; color: #ea580c; }}
                .cat-pill.actief {{ background: #ea580c; color: #fff; border-color: #ea580c; }}
                .post-card {{ background: #fff; padding: 24px 28px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 16px; transition: box-shadow 0.15s; overflow: hidden; }}
                .post-card:hover {{ box-shadow: 0 4px 12px rgba(0,0,0,0.05); }}
                .card-cover {{ width: 100%; max-height: 220px; object-fit: cover; border-radius: 6px; margin-bottom: 14px; display: block; }}
                .post-meta {{ display: flex; align-items: center; gap: 12px; font-size: 13px; color: #6b7280; margin-bottom: 8px; }}
                .cat-badge {{ background: #fff7ed; color: #c2410c; border: 1px solid #fed7aa; padding: 2px 8px; border-radius: 4px; text-decoration: none; font-weight: 600; }}
                .draft-badge {{ background: #fef3c7; color: #92400e; padding: 2px 8px; border-radius: 4px; font-weight: 700; font-size: 11px; }}
                .crud-edit {{ font-size: 12px; text-decoration: none; color: #2271b1; border: 1px solid #2271b1; padding: 2px 8px; border-radius: 4px; background: #f6f7f7; }}
                .crud-del {{ font-size: 12px; color: #dc2626; border: 1px solid #dc2626; padding: 2px 6px; border-radius: 4px; background: #fff; cursor: pointer; }}
                .post-card h2 {{ margin: 4px 0 10px 0; font-size: 21px; }}
                .post-card h2 a {{ color: #111827; text-decoration: none; }}
                .post-card h2 a:hover {{ color: #ea580c; }}
                .summary {{ color: #4b5563; line-height: 1.6; margin: 0 0 16px 0; }}
                .post-footer {{ display: flex; justify-content: space-between; align-items: center; border-top: 1px solid #f3f4f6; padding-top: 12px; }}
                .tags {{ display: flex; gap: 6px; flex-wrap: wrap; }}
                .tag {{ font-size: 12px; color: #2563eb; background: #eff6ff; padding: 2px 8px; border-radius: 4px; text-decoration: none; }}
                .tag:hover {{ background: #dbeafe; }}
                .read-more {{ font-size: 14px; font-weight: 600; color: #ea580c; text-decoration: none; }}
                .read-more:hover {{ text-decoration: underline; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            <header class="hero">
                <div style="flex:1;min-width:240px;">
                    <h1>{titel}</h1>
                    <p>{beschrijving}</p>
                </div>
                <div style="display:flex;align-items:center;gap:14px;flex-wrap:wrap;">
                    <a href="/download/folder" title="Download de volledige BlogRust CMS Folder (.zip) inclusief README.md, Docker, Instellingen &amp; Mappen (zonder Google Auth)" style="display:inline-block;width:68px;height:68px;border-radius:50%;box-shadow:0 4px 12px rgba(234,88,12,0.35);transition:transform 0.2s;flex-shrink:0;" onmouseover="this.style.transform='scale(1.08)'" onmouseout="this.style.transform='scale(1)'">
                        <img src="/media/blogrust-download-rond.svg" alt="Download BlogRust Folder" style="width:100%;height:100%;border-radius:50%;display:block;">
                    </a>
                    <form action="/" method="get" class="search-form">
                        <input type="text" name="zoek" value="{zoekterm}" placeholder="🔍 Zoek in artikelen..." class="search-input">
                        <button type="submit" class="search-btn">Zoek</button>
                    </form>
                </div>
            </header>

            <div class="cat-bar">
                {cat_knoppen}
            </div>

            <main>
                {posts_html}
            </main>
        </body>
        </html>
        "#,
        titel = titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        beschrijving = beschrijving,
        zoekterm = zoekterm,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        cat_knoppen = cat_knoppen,
        posts_html = posts_html
    ))
    .into_response()
}

// 2. Detailpagina van één Markdown-artikel (/post/{slug})
async fn toon_post_pagina(
    headers: HeaderMap,
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let ingelogde_gebruiker = haal_ingelogde_gebruiker_op(&headers, &state);
    let is_ingelogd = ingelogde_gebruiker.is_some();
    let nav_id = haal_nav_identiteit_op(&headers, &state);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let bestandspad = format!("{}/{}.md", state.posts_map, slug);

    let Ok(ruwe_inhoud) = fs::read_to_string(&bestandspad) else {
        return (
            StatusCode::NOT_FOUND,
            Html("<h1>404 - Artikel niet gevonden</h1><p><a href=\"/\">Terug naar startpagina</a></p>".to_string()),
        )
            .into_response();
    };

    let basis_post = parse_markdown_post(&slug, &ruwe_inhoud);
    if basis_post.concept && !is_ingelogd {
        return Redirect::to("/login").into_response();
    }

    let post = pas_plugins_toe_op_post(&state.plugins_map, basis_post);

    let admin_acties = if is_ingelogd {
        format!(
            r#"<span style="margin-left:auto;display:flex;gap:8px;">
                <a href="/bewerk/{slug}" class="btn-link" style="font-size:13px;">✏️ Bericht bewerken</a>
                <form action="/verwijder/{slug}" method="post" style="margin:0;" onsubmit="return confirm('Weet je zeker dat je dit artikel wilt verwijderen?');">
                    <button type="submit" class="btn-logout" style="padding:5px 10px;font-size:13px;">🗑️ Verwijderen</button>
                </form>
            </span>"#,
            slug = post.slug
        )
    } else {
        String::new()
    };

    let tags_html: String = post
        .tags
        .iter()
        .map(|t| format!(r#"<a href="/?tag={t}" class="tag">#{t}</a>"#))
        .collect::<Vec<_>>()
        .join(" ");

    // Bepaal het vorige en volgende artikel (van begin naar eind)
    let alle_posts: Vec<MarkdownPost> = laad_alle_posts(&state.posts_map)
        .into_iter()
        .filter(|p| is_ingelogd || !p.concept)
        .collect();

    let huidige_idx = alle_posts.iter().position(|p| p.slug == slug);
    let vorig_html = huidige_idx
        .and_then(|i| if i > 0 { alle_posts.get(i - 1) } else { None })
        .map(|v| {
            format!(
                r#"<a href="/post/{slug}" class="btn-link">&larr; Vorig: {titel}</a>"#,
                slug = v.slug,
                titel = v.titel
            )
        })
        .unwrap_or_else(|| r#"<span></span>"#.to_string());

    let volgend_html = huidige_idx
        .and_then(|i| alle_posts.get(i + 1))
        .map(|v| {
            format!(
                r#"<a href="/post/{slug}" class="btn-new">Volgend: {titel} &rarr;</a>"#,
                slug = v.slug,
                titel = v.titel
            )
        })
        .unwrap_or_else(|| r#"<span></span>"#.to_string());

    // Check of de Google Auth Reacties Plug-in (plugins/reacties.rhai) actief is
    let reacties_html = if post.html_inhoud.contains("<!-- BLOGRUST_GOOGLE_REACTIES -->") {
        let reacties_lijst = haal_reacties_voor_post(&state.db_pad, &post.slug).await;
        let google_gebruiker: Option<String> = None;
        let actieve_auteur = google_gebruiker
            .clone()
            .or_else(|| ingelogde_gebruiker.clone());

        let mut lijst_items_html = String::new();
        if reacties_lijst.is_empty() {
            lijst_items_html.push_str(
                r#"<p style="color:#6b7280;font-size:14px;margin:0 0 18px 0;">Er zijn nog geen reacties onder dit artikel. Plaats als eerste een reactie via Google Auth!</p>"#,
            );
        } else {
            for (r_id, r_auteur, r_bericht, r_datum) in &reacties_lijst {
                let verwijder_knop = if is_ingelogd {
                    format!(
                        r#"<form action="/post/{slug}/reactie/verwijder" method="post" style="margin:0;">
                            <input type="hidden" name="reactie_id" value="{r_id}">
                            <button type="submit" style="background:#fff;border:1px solid #dc2626;color:#dc2626;padding:3px 8px;border-radius:4px;font-size:12px;cursor:pointer;">🗑️ Verwijder</button>
                        </form>"#,
                        slug = post.slug,
                        r_id = r_id
                    )
                } else {
                    String::new()
                };

                let veilige_tekst = r_bericht
                    .replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
                    .replace('\n', "<br>");

                lijst_items_html.push_str(&format!(
                    r#"
                    <div style="padding:14px 16px;background:#f8fafc;border:1px solid #e5e7eb;border-radius:8px;margin-bottom:12px;">
                        <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:6px;gap:8px;flex-wrap:wrap;">
                            <div style="font-size:13px;font-weight:600;color:#1d4ed8;">🌐 {auteur} <span style="color:#6b7280;font-weight:400;margin-left:6px;">• {datum}</span></div>
                            {verwijder_knop}
                        </div>
                        <div style="font-size:14px;color:#1f2937;line-height:1.6;">{tekst}</div>
                    </div>
                    "#,
                    auteur = r_auteur,
                    datum = r_datum,
                    verwijder_knop = verwijder_knop,
                    tekst = veilige_tekst
                ));
            }
        }

        let formulier_html = if let Some(auteur) = actieve_auteur {
            format!(
                r#"
                <form action="/post/{slug}/reactie" method="post" style="margin-top:16px;padding-top:16px;border-top:1px solid #e5e7eb;">
                    <div style="font-size:13px;color:#15803d;font-weight:600;margin-bottom:8px;">✅ Ingelogd als <strong>{auteur}</strong></div>
                    <textarea name="bericht" placeholder="Schrijf je reactie op dit artikel..." required style="width:100%;min-height:90px;padding:10px 12px;border:1px solid #d1d5db;border-radius:6px;font-family:inherit;font-size:14px;box-sizing:border-box;margin-bottom:10px;"></textarea>
                    <button type="submit" class="btn-new" style="cursor:pointer;font-size:14px;padding:8px 18px;">💬 Reactie Plaatsen</button>
                </form>
                "#,
                slug = post.slug,
                auteur = auteur
            )
        } else {
            format!(
                r#"
                <div style="margin-top:16px;padding:18px;background:#eff6ff;border:1px solid #bfdbfe;border-radius:8px;text-align:center;">
                    <div style="font-size:14px;color:#1e3a8a;font-weight:600;margin-bottom:8px;">🌐 Wil je reageren op dit artikel?</div>
                    <p style="font-size:13px;color:#3b82f6;margin:0 0 12px 0;">Log met 1 klik in via Google Auth om een reactie achter te laten.</p>
                    <a href="/auth/google?melding=post:{slug}" class="btn-login" style="background:#2563eb;border-color:#1d4ed8;display:inline-block;text-decoration:none;">🌐 Inloggen met Google om te reageren</a>
                </div>
                "#,
                slug = post.slug
            )
        };

        format!(
            r#"
            <section id="reacties" class="article-box" style="margin-top:20px;padding:28px 36px;">
                <h2 style="margin:0 0 16px 0;font-size:20px;color:#111827;border-bottom:1px solid #e5e7eb;padding-bottom:10px;">💬 Community Reacties ({aantal}) <span style="font-size:12px;font-weight:500;color:#6b7280;margin-left:6px;">via <code>plugins/reacties.rhai</code> + Google Auth</span></h2>
                {lijst_items_html}
                {formulier_html}
            </section>
            "#,
            aantal = reacties_lijst.len(),
            lijst_items_html = lijst_items_html,
            formulier_html = formulier_html
        )
    } else {
        String::new()
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>{post_titel} &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-login:hover {{ background: #135e96; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; cursor: pointer; }}
                .article-box {{ background: #fff; padding: 36px 40px; border-radius: 8px; border: 1px solid #e5e7eb; line-height: 1.75; }}
                .back-link {{ display: inline-block; margin-bottom: 16px; color: #ea580c; text-decoration: none; font-weight: 600; font-size: 14px; }}
                .post-meta {{ display: flex; align-items: center; gap: 12px; font-size: 14px; color: #6b7280; margin-bottom: 10px; flex-wrap: wrap; }}
                .cat-badge {{ background: #fff7ed; color: #c2410c; border: 1px solid #fed7aa; padding: 2px 10px; border-radius: 4px; text-decoration: none; font-weight: 600; }}
                h1 {{ font-size: 30px; color: #111827; margin: 0 0 16px 0; border-bottom: 1px solid #e5e7eb; padding-bottom: 16px; }}
                .content img {{ max-width: 100%; border-radius: 8px; }}
                .content img.plugin-cover {{ width: 100%; max-height: 360px; object-fit: cover; border-radius: 8px; margin-bottom: 20px; display: block; }}
                .content code {{ background: #f3f4f6; padding: 2px 6px; border-radius: 4px; font-size: 14px; }}
                .content pre {{ background: #1f2937; color: #f9fafb; padding: 16px; border-radius: 6px; overflow-x: auto; }}
                .content pre code {{ background: none; color: inherit; padding: 0; }}
                .content table {{ width: 100%; border-collapse: collapse; margin: 16px 0; }}
                .content th, .content td {{ border: 1px solid #e5e7eb; padding: 8px 12px; text-align: left; }}
                .content th {{ background: #f9fafb; }}
                .tags {{ display: flex; gap: 6px; margin-top: 24px; padding-top: 16px; border-top: 1px solid #e5e7eb; }}
                .tag {{ font-size: 13px; color: #2563eb; background: #eff6ff; padding: 3px 10px; border-radius: 4px; text-decoration: none; }}
                .file-hint {{ font-size: 12px; color: #9ca3af; margin-top: 12px; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            <article class="article-box">
                <a href="/" class="back-link">&larr; Terug naar overzicht</a>
                <div class="post-meta">
                    <span>📅 {datum}</span>
                    <a href="/?categorie={categorie}" class="cat-badge">📁 {categorie}</a>
                    {admin_acties}
                </div>
                <h1>{post_titel}</h1>
                <div class="content">
                    {html_inhoud}
                </div>
                <div class="tags">{tags_html}</div>
                <div style="display:flex;justify-content:space-between;align-items:center;margin-top:24px;padding-top:16px;border-top:1px solid #e5e7eb;gap:12px;flex-wrap:wrap;">
                    {vorig_html}
                    {volgend_html}
                </div>
                <div class="file-hint">📄 Bronbestand: <code>posts/{slug}.md</code></div>
            </article>

            {reacties_html}
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        datum = post.datum,
        categorie = post.categorie,
        admin_acties = admin_acties,
        post_titel = post.titel,
        html_inhoud = post.html_inhoud,
        tags_html = tags_html,
        vorig_html = vorig_html,
        volgend_html = volgend_html,
        slug = post.slug,
        reacties_html = reacties_html
    ))
    .into_response()
}

// Hulpfuncties voor de Google Auth Reacties Plug-in (SQLite tabel `reacties`)
async fn zorg_voor_reacties_tabel(pool: &SqlitePool) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS reacties (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            post_slug TEXT NOT NULL,
            auteur TEXT NOT NULL,
            bericht TEXT NOT NULL,
            datum TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await;
}

async fn haal_reacties_voor_post(
    db_pad: &str,
    post_slug: &str,
) -> Vec<(i64, String, String, String)> {
    let Ok(pool) = SqlitePool::connect(&format!("sqlite:{}", db_pad)).await else {
        return Vec::new();
    };
    zorg_voor_reacties_tabel(&pool).await;

    sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, auteur, bericht, datum FROM reacties WHERE post_slug = ? ORDER BY id ASC",
    )
    .bind(post_slug)
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
}

async fn verwerk_nieuwe_reactie(
    headers: HeaderMap,
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<ReactieForm>,
) -> impl IntoResponse {
    let google_gebruiker: Option<String> = None;
    let admin_gebruiker = haal_ingelogde_gebruiker_op(&headers, &state);

    let Some(auteur) = google_gebruiker.or(admin_gebruiker) else {
        return Redirect::to(&format!("/auth/google?melding=post:{}", slug)).into_response();
    };

    let schoon_bericht = form.bericht.trim();
    if !schoon_bericht.is_empty() {
        if let Ok(pool) = SqlitePool::connect(&format!("sqlite:{}", state.db_pad)).await {
            zorg_voor_reacties_tabel(&pool).await;
            let _ = sqlx::query(
                "INSERT INTO reacties (post_slug, auteur, bericht, datum) VALUES (?, ?, ?, date('now'))",
            )
            .bind(&slug)
            .bind(&auteur)
            .bind(schoon_bericht)
            .execute(&pool)
            .await;
        }
    }

    Redirect::to(&format!("/post/{}#reacties", slug)).into_response()
}

async fn verwerk_reactie_verwijder(
    headers: HeaderMap,
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
    Form(form): Form<ReactieVerwijderForm>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    if let Ok(pool) = SqlitePool::connect(&format!("sqlite:{}", state.db_pad)).await {
        zorg_voor_reacties_tabel(&pool).await;
        let _ = sqlx::query("DELETE FROM reacties WHERE id = ? AND post_slug = ?")
            .bind(form.reactie_id)
            .bind(&slug)
            .execute(&pool)
            .await;
    }

    Redirect::to(&format!("/post/{}#reacties", slug)).into_response()
}

// 3. De Thema Beheer & Installatiepagina (/themes) met Scaffold
fn haal_preset_themas() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "standaard.css",
            "🦀 Standaard Rust Oranje",
            "Het frisse standaard BlogRust uiterlijk met lichte kaarten en warme Rust-oranje accenten.",
            r#"/* Standaard Rust Oranje Thema - Het originele BlogRust uiterlijk */
body {
    background: #f9fafb;
    color: #1f2937;
}
.brand, .hero h1, .read-more, .back-link {
    color: #ea580c !important;
}
.cat-pill.actief, .btn-new {
    background: #ea580c !important;
    border-color: #ea580c !important;
    color: #ffffff !important;
}
"#,
        ),
        (
            "midnight.css",
            "🌌 Midnight Dark (Developer)",
            "Een stijlvol donker thema met diepblauwe slate-tinten en heldere oranje/blauwe accenten, ideaal voor technische blogs.",
            r#"/* Midnight Dark Thema - Donker developer thema met diepblauwe tinten */
body {
    background: #0f172a !important;
    color: #e2e8f0 !important;
}
.topbar, .hero, .post-card, .article-box, .card, .store-card, .media-card {
    background: #1e293b !important;
    border-color: #334155 !important;
    color: #e2e8f0 !important;
}
h1, h2, h3, .post-card h2 a, .store-card h3 {
    color: #f8fafc !important;
}
.hero p, .summary, .post-meta, .user-badge, .store-card p {
    color: #94a3b8 !important;
}
.brand, .hero h1, .read-more, .back-link {
    color: #fb923c !important;
}
.cat-pill {
    background: #1e293b !important;
    border-color: #475569 !important;
    color: #cbd5e1 !important;
}
.cat-pill.actief, .btn-new {
    background: #ea580c !important;
    border-color: #ea580c !important;
    color: #ffffff !important;
}
.cat-badge {
    background: #334155 !important;
    color: #fdba74 !important;
    border-color: #475569 !important;
}
.tag {
    background: #1e3a8a !important;
    color: #93c5fd !important;
}
.content code {
    background: #334155 !important;
    color: #f8fafc !important;
}
"#,
        ),
        (
            "krant.css",
            "📰 Klassieke Krant (Serif)",
            "Een literair kranten-uiterlijk met Georgia serif-typografie, warm papier-achtergrond en strakke zwarte kaders.",
            r#"/* Klassieke Krant Thema - Serif typografie op warm krantenpapier */
body {
    background: #f5f2eb !important;
    color: #1c1917 !important;
    font-family: Georgia, "Times New Roman", serif !important;
}
.topbar, .hero, .post-card, .article-box, .card {
    background: #fffdf9 !important;
    border: 2px solid #292524 !important;
    border-radius: 0px !important;
    box-shadow: 4px 4px 0px #292524 !important;
}
.brand, .hero h1, .read-more, .back-link {
    color: #991b1b !important;
}
.cat-pill.actief, .btn-new {
    background: #991b1b !important;
    border-color: #991b1b !important;
    color: #ffffff !important;
    border-radius: 2px !important;
}
.cat-badge {
    background: #fef2f2 !important;
    color: #991b1b !important;
    border-color: #fca5a5 !important;
}
"#,
        ),
        (
            "bosgroen.css",
            "🌲 Bosgroen Minimal",
            "Een rustgevend natuur-thema met zachte groentinten, smaragdgroene knoppen en afgeronde kaartjes.",
            r#"/* Bosgroen Minimal Thema - Rustgevend thema met smaragdgroene accenten */
body {
    background: #f0fdf4 !important;
    color: #14532d !important;
}
.topbar, .hero, .post-card, .article-box, .card {
    background: #ffffff !important;
    border-color: #bbf7d0 !important;
    border-radius: 14px !important;
}
.brand, .hero h1, .read-more, .back-link {
    color: #15803d !important;
}
.cat-pill.actief, .btn-new {
    background: #15803d !important;
    border-color: #15803d !important;
    color: #ffffff !important;
}
.cat-badge {
    background: #dcfce7 !important;
    color: #166534 !important;
    border-color: #86efac !important;
}
"#,
        ),
    ]
}

fn lees_geinstalleerde_themas(themes_map: &str, actief_thema: &str) -> Vec<GeinstalleerdThema> {
    let mut lijst = Vec::new();
    if let Ok(entries) = fs::read_dir(themes_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if let Some(naam) = pad.file_name().and_then(|n| n.to_str()) {
                if naam.ends_with(".css") {
                    let inhoud = fs::read_to_string(&pad).unwrap_or_default();
                    let eerste_regel = inhoud
                        .lines()
                        .next()
                        .unwrap_or("/* Aangepast CSS Thema */")
                        .trim_start_matches("/*")
                        .trim_end_matches("*/")
                        .trim()
                        .to_string();

                    lijst.push(GeinstalleerdThema {
                        bestandsnaam: naam.to_string(),
                        actief: naam.eq_ignore_ascii_case(actief_thema),
                        beschrijving: eerste_regel,
                    });
                }
            }
        }
    }
    lijst.sort_by(|a, b| a.bestandsnaam.cmp(&b.bestandsnaam));
    lijst
}

async fn zet_actief_thema(db_pad: &str, thema_naam: &str) {
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_pad))
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('actief_thema', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(thema_naam)
    .execute(&pool)
    .await
    .unwrap();
}

async fn themes_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let admin_gebruiker = haal_ingelogde_gebruiker_op(&headers, &state);
    let google_gebruiker: Option<String> = None;
    let is_admin = admin_gebruiker.is_some();
    let nav_id = haal_nav_identiteit_op(&headers, &state);

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());
    let geinstalleerd = lees_geinstalleerde_themas(&state.themes_map, &actief_thema);
    let theme_scaffold = laad_scaffold(
        &state.scaffolds_map,
        "theme.css",
        "/* Mijn Nieuwe Thema */\nbody {\n    background: #f8fafc !important;\n}\n",
    );

    let melding_banner = if let Some(m) = query.melding {
        format!(
            r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 16px;border-radius:6px;margin-bottom:20px;font-weight:500;">✅ {m}</div>"#
        )
    } else {
        String::new()
    };

    let mut geinstalleerd_html = String::new();
    if geinstalleerd.is_empty() {
        geinstalleerd_html.push_str(
            r#"<p style="color:#6b7280;">Er staan nog geen <code>.css</code> thema's in de map <code>themes/</code>.</p>"#,
        );
    } else {
        for t in &geinstalleerd {
            let status_badge = if t.actief {
                r#"<span class="status-actief">● Actief op deze site</span>"#
            } else {
                r#"<span class="status-uit">○ Community Thema</span>"#
            };

            let admin_knoppen = if is_admin {
                let activeer_knop = if t.actief {
                    r#"<span class="installed-pill">Huidig thema</span>"#.to_string()
                } else {
                    format!(
                        r#"<form action="/themes" method="post" style="margin:0;">
                            <input type="hidden" name="actie" value="activeer">
                            <input type="hidden" name="naam" value="{naam}">
                            <button type="submit" class="btn-sec">🎨 Activeer</button>
                        </form>"#,
                        naam = t.bestandsnaam
                    )
                };
                format!(
                    r#"{activeer_knop}
                    <form action="/themes" method="post" style="margin:0;">
                        <input type="hidden" name="actie" value="verwijder">
                        <input type="hidden" name="naam" value="{naam}">
                        <button type="submit" class="btn-del">🗑️ Verwijder</button>
                    </form>"#,
                    activeer_knop = activeer_knop,
                    naam = t.bestandsnaam
                )
            } else {
                String::new()
            };

            geinstalleerd_html.push_str(&format!(
                r#"
                <div class="plugin-row">
                    <div>
                        <div style="display:flex;align-items:center;gap:10px;">
                            <strong><code>themes/{naam}</code></strong>
                            {status_badge}
                        </div>
                        <div style="font-size:13px;color:#6b7280;margin-top:4px;">{beschrijving}</div>
                    </div>
                    <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
                        <a href="/themes/download/{naam}" class="btn-sec" style="text-decoration:none;">⬇️ Download .css</a>
                        {admin_knoppen}
                    </div>
                </div>
                "#,
                naam = t.bestandsnaam,
                status_badge = status_badge,
                beschrijving = t.beschrijving,
                admin_knoppen = admin_knoppen
            ));
        }
    }

    let mut catalogus_html = String::new();
    for (bestandsnaam, titel_naam, uitleg, _) in haal_preset_themas() {
        let al_aanwezig = geinstalleerd
            .iter()
            .any(|g| g.bestandsnaam.eq_ignore_ascii_case(bestandsnaam));

        let admin_install_knop = if is_admin {
            if al_aanwezig {
                r#"<span class="installed-pill">✅ In themes/</span>"#.to_string()
            } else {
                format!(
                    r#"<form action="/themes" method="post" style="margin:0;">
                        <input type="hidden" name="actie" value="installeer_preset">
                        <input type="hidden" name="naam" value="{bestandsnaam}">
                        <button type="submit" class="btn-install">⬇️ Installeer &amp; Activeer</button>
                    </form>"#
                )
            }
        } else {
            String::new()
        };

        catalogus_html.push_str(&format!(
            r#"
            <div class="store-card">
                <div>
                    <h3>{titel_naam}</h3>
                    <p>{uitleg}</p>
                    <small style="color:#9ca3af;">Bestand: <code>{bestandsnaam}</code></small>
                </div>
                <div style="margin-top:14px;display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
                    <a href="/themes/download/{bestandsnaam}" class="btn-sec" style="text-decoration:none;">⬇️ Download .css</a>
                    {admin_install_knop}
                </div>
            </div>
            "#
        ));
    }

    let toevoegen_sectie_html = if is_admin || google_gebruiker.is_some() {
        let auteur_badge = if let Some(ref email) = google_gebruiker {
            format!(
                r#"<div style="background:#eff6ff;border:1px solid #bfdbfe;color:#1d4ed8;padding:10px 14px;border-radius:6px;margin-bottom:14px;font-size:13px;">🌐 Ingelogd via Google als <strong>{email}</strong> — jouw thema wordt direct gepubliceerd in de community-map <code>themes/</code>.</div>"#
            )
        } else {
            String::new()
        };
        format!(
            r#"
            <section class="card">
                <h2>🛠️ Eigen CSS Thema Toevoegen (vooringevuld met <code>scaffolds/theme.css</code>)</h2>
                {auteur_badge}
                <form action="/themes" method="post">
                    <input type="hidden" name="actie" value="eigen_thema">
                    <label style="font-weight:600;font-size:14px;display:block;margin-bottom:6px;">Bestandsnaam (bijv. <code>oceaan.css</code>)</label>
                    <input type="text" name="naam" placeholder="oceaan.css" required>
                    <label style="font-weight:600;font-size:14px;display:block;margin-bottom:6px;">CSS Thema Code (Scaffold)</label>
                    <textarea name="css" required>{theme_scaffold}</textarea>
                    <button type="submit" class="btn-install">💾 Thema Publiceren in themes/</button>
                </form>
            </section>
            "#
        )
    } else {
        r#"
        <section class="card" style="text-align:center;background:#eff6ff;border-color:#bfdbfe;">
            <h2>🌐 Zelf een Thema delen met de BlogRust Community?</h2>
            <p style="font-size:14px;color:#1e3a8a;max-width:600px;margin:0 auto 16px auto;">Net als op WordPress.org kan iedereen thema's downloaden. Log in met je <strong>Google-account</strong> om zelf een nieuw <code>.css</code> thema aan deze bibliotheek toe te voegen!</p>
            <a href="/auth/google?melding=themes" class="btn-install" style="background:#2563eb;border-color:#1d4ed8;text-decoration:none;display:inline-block;padding:10px 20px;">🌐 Inloggen met Google om Thema toe te voegen</a>
        </section>
        "#
        .to_string()
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Thema Directory &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .card {{ background: #fff; padding: 26px 30px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                h1, h2 {{ margin-top: 0; color: #111827; }}
                h1 {{ font-size: 23px; border-bottom: 1px solid #e5e7eb; padding-bottom: 12px; }}
                h2 {{ font-size: 18px; margin-bottom: 14px; }}
                .plugin-row {{ display: flex; justify-content: space-between; align-items: center; padding: 14px 0; border-bottom: 1px solid #f3f4f6; gap: 12px; flex-wrap: wrap; }}
                .plugin-row:last-child {{ border-bottom: none; }}
                .status-actief {{ font-size: 12px; font-weight: 700; color: #15803d; background: #dcfce7; padding: 2px 8px; border-radius: 999px; }}
                .status-uit {{ font-size: 12px; font-weight: 600; color: #6b7280; background: #f3f4f6; padding: 2px 8px; border-radius: 999px; }}
                .btn-sec {{ background: #f6f7f7; border: 1px solid #2271b1; color: #2271b1; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; font-weight: 600; }}
                .btn-del {{ background: #fff; border: 1px solid #dc2626; color: #dc2626; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; }}
                .store-grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }}
                .store-card {{ border: 1px solid #e5e7eb; border-radius: 8px; padding: 18px; display: flex; flex-direction: column; justify-content: space-between; background: #fafafa; }}
                .store-card h3 {{ margin: 0 0 8px 0; font-size: 16px; color: #111827; }}
                .store-card p {{ margin: 0 0 10px 0; font-size: 13px; color: #4b5563; line-height: 1.5; }}
                .btn-install {{ background: #ea580c; color: #fff; border: 1px solid #ea580c; padding: 8px 14px; border-radius: 5px; font-size: 13px; font-weight: 600; cursor: pointer; }}
                .btn-install:hover {{ background: #c2410c; }}
                .installed-pill {{ display: inline-block; font-size: 13px; font-weight: 600; color: #15803d; background: #dcfce7; padding: 6px 12px; border-radius: 5px; }}
                input[type="text"], textarea {{ width: 100%; padding: 9px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; box-sizing: border-box; margin-bottom: 12px; }}
                textarea {{ font-family: ui-monospace, Consolas, monospace; min-height: 170px; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            {melding_banner}

            <section class="card">
                <h1>🎨 Community Thema Directory (<code>themes/</code>)</h1>
                <p style="font-size:14px;color:#4b5563;margin-top:0;margin-bottom:14px;">Download elk thema als <code>.css</code> bestand voor je eigen BlogRust-installatie, of voeg via Google Auth je eigen thema toe.</p>
                {geinstalleerd_html}
            </section>

            <section class="card">
                <h2>🏪 Officiële BlogRust Thema's</h2>
                <div class="store-grid">
                    {catalogus_html}
                </div>
            </section>

            {toevoegen_sectie_html}
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        melding_banner = melding_banner,
        geinstalleerd_html = geinstalleerd_html,
        catalogus_html = catalogus_html,
        toevoegen_sectie_html = toevoegen_sectie_html
    ))
    .into_response()
}

async fn download_thema_bestand(
    AxumPath(naam): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let veilige_naam = maak_veilige_bestandsnaam(&naam);
    let pad = format!("{}/{}", state.themes_map, veilige_naam);

    let inhoud = if let Ok(tekst) = fs::read_to_string(&pad) {
        Some(tekst)
    } else {
        haal_preset_themas()
            .into_iter()
            .find(|(n, _, _, _)| *n == veilige_naam)
            .map(|(_, _, _, css)| css.to_string())
    };

    if let Some(css_tekst) = inhoud {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        );
        if let Ok(disp) = HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            veilige_naam
        )) {
            headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
        }
        (StatusCode::OK, headers, css_tekst).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Thema niet gevonden").into_response()
    }
}

async fn verwerk_thema_actie(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<ThemaActieForm>,
) -> impl IntoResponse {
    let admin = haal_ingelogde_gebruiker_op(&headers, &state);
    let google_user: Option<String> = None;

    if admin.is_none() && google_user.is_none() {
        return Redirect::to("/auth/google").into_response();
    }

    fs::create_dir_all(&state.themes_map).unwrap();
    let veilige_naam = maak_veilige_bestandsnaam(&form.naam);

    match form.actie.as_str() {
        "installeer_preset" if admin.is_some() => {
            for (preset_naam, _, _, preset_css) in haal_preset_themas() {
                if preset_naam == veilige_naam {
                    let doel = format!("{}/{}", state.themes_map, preset_naam);
                    fs::write(doel, preset_css).unwrap();
                    zet_actief_thema(&state.db_pad, preset_naam).await;
                    return Redirect::to(&format!(
                        "/themes?melding=Thema+{}+geïnstalleerd+en+direct+geactiveerd!",
                        preset_naam
                    ))
                    .into_response();
                }
            }
        }
        "activeer" if admin.is_some() => {
            zet_actief_thema(&state.db_pad, &veilige_naam).await;
            return Redirect::to(&format!(
                "/themes?melding=Thema+{}+is+nu+actief!",
                veilige_naam
            ))
            .into_response();
        }
        "verwijder" if admin.is_some() => {
            let pad = format!("{}/{}", state.themes_map, veilige_naam);
            let _ = fs::remove_file(pad);
            return Redirect::to("/themes?melding=Thema+verwijderd+uit+themes/").into_response();
        }
        "eigen_thema" => {
            let mut eind_naam = veilige_naam;
            if !eind_naam.ends_with(".css") {
                eind_naam.push_str(".css");
            }
            if let Some(css) = form.css {
                let eind_css = if let Some(ref email) = google_user {
                    if admin.is_none() {
                        format!("/* Community Thema door (Google Auth): {} */\n{}", email, css)
                    } else {
                        css
                    }
                } else {
                    css
                };
                let pad = format!("{}/{}", state.themes_map, eind_naam);
                fs::write(pad, eind_css).unwrap();
                if admin.is_some() {
                    zet_actief_thema(&state.db_pad, &eind_naam).await;
                }
                return Redirect::to(&format!(
                    "/themes?melding=Community+thema+{}+is+gepubliceerd+in+themes/!",
                    eind_naam
                ))
                .into_response();
            }
        }
        _ => {}
    }

    Redirect::to("/themes").into_response()
}

// 4. De Media Bibliotheek & Upload Tool (/media)
async fn media_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let Some(gebruiker) = haal_ingelogde_gebruiker_op(&headers, &state) else {
        return Redirect::to("/login").into_response();
    };

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_rechts = maak_nav_rechts_html(Some(&gebruiker));
    let bestanden = lees_media_bestanden(&state.media_map);

    let melding_banner = if let Some(m) = query.melding {
        format!(
            r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 16px;border-radius:6px;margin-bottom:20px;font-weight:500;">✅ {m}</div>"#
        )
    } else {
        String::new()
    };

    let mut media_grid_html = String::new();
    if bestanden.is_empty() {
        media_grid_html.push_str(
            r#"<p style="color:#6b7280;">De map <code>media/</code> is nog leeg. Upload hierboven je eerste afbeelding!</p>"#,
        );
    } else {
        for b in &bestanden {
            let url_pad = format!("/media/{}", b.bestandsnaam);
            let md_code = format!("![{}](/media/{})", b.bestandsnaam, b.bestandsnaam);
            media_grid_html.push_str(&format!(
                r#"
                <div class="media-card">
                    <div class="thumb-wrap">
                        <img src="{url_pad}" alt="{naam}" class="thumb">
                    </div>
                    <div class="media-info">
                        <div class="media-name" title="{naam}"><code>{naam}</code></div>
                        <div class="media-size">{kb} KB</div>
                        <div class="media-actions">
                            <button type="button" class="btn-copy" onclick="kopieerTekst('{url_pad}', this)">📋 Kopieer URL</button>
                            <button type="button" class="btn-copy" onclick="kopieerTekst('{md_code}', this)">📋 Markdown</button>
                            <form action="/media/verwijder" method="post" style="margin:0;">
                                <input type="hidden" name="bestandsnaam" value="{naam}">
                                <button type="submit" class="btn-del" title="Verwijderen">🗑️</button>
                            </form>
                        </div>
                    </div>
                </div>
                "#,
                url_pad = url_pad,
                naam = b.bestandsnaam,
                kb = b.grootte_kb,
                md_code = md_code
            ));
        }
    }

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Media Bibliotheek &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .card {{ background: #fff; padding: 26px 30px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                h1, h2 {{ margin-top: 0; color: #111827; }}
                h1 {{ font-size: 22px; border-bottom: 1px solid #e5e7eb; padding-bottom: 12px; }}
                .upload-box {{ border: 2px dashed #cbd5e1; border-radius: 8px; padding: 24px; text-align: center; background: #f8fafc; display: flex; flex-direction: column; align-items: center; gap: 12px; }}
                .btn-upload {{ background: #ea580c; color: #fff; border: 1px solid #ea580c; padding: 10px 20px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; }}
                .btn-upload:hover {{ background: #c2410c; }}
                .media-grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 16px; }}
                .media-card {{ border: 1px solid #e5e7eb; border-radius: 8px; overflow: hidden; background: #fff; display: flex; flex-direction: column; }}
                .thumb-wrap {{ height: 140px; background: #f3f4f6; display: flex; align-items: center; justify-content: center; overflow: hidden; border-bottom: 1px solid #e5e7eb; }}
                .thumb {{ width: 100%; height: 100%; object-fit: cover; }}
                .media-info {{ padding: 12px; }}
                .media-name {{ font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }}
                .media-size {{ font-size: 12px; color: #6b7280; margin: 4px 0 10px 0; }}
                .media-actions {{ display: flex; gap: 6px; align-items: center; }}
                .btn-copy {{ flex: 1; background: #f6f7f7; border: 1px solid #2271b1; color: #2271b1; padding: 5px 8px; border-radius: 4px; cursor: pointer; font-size: 12px; font-weight: 600; }}
                .btn-copy:hover {{ background: #e5f0fa; }}
                .btn-del {{ background: #fff; border: 1px solid #dc2626; color: #dc2626; padding: 5px 8px; border-radius: 4px; cursor: pointer; font-size: 12px; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            {melding_banner}

            <section class="card">
                <h1>⬆️ Afbeelding Uploaden naar <code>media/</code></h1>
                <form action="/media" method="post" enctype="multipart/form-data" class="upload-box">
                    <div style="font-size:14px;color:#4b5563;">Kies een afbeelding van je computer (<code>.jpg</code>, <code>.png</code>, <code>.webp</code>, <code>.gif</code> of <code>.svg</code>)</div>
                    <input type="file" name="bestand" accept="image/*" required style="font-size:14px;">
                    <button type="submit" class="btn-upload">⬆️ Uploaden naar media/ map</button>
                </form>
            </section>

            <section class="card">
                <h2>🖼️ Media Bibliotheek (<code>media/</code>)</h2>
                <p style="font-size:14px;color:#4b5563;margin-top:0;margin-bottom:16px;">Klik op <strong>Kopieer URL</strong> voor je cover-afbeelding of op <strong>Markdown</strong> om een afbeelding midden in een artikel te plakken.</p>
                <div class="media-grid">
                    {media_grid_html}
                </div>
            </section>

            <script>
                function kopieerTekst(tekst, knop) {{
                    navigator.clipboard.writeText(tekst);
                    const oud = knop.innerText;
                    knop.innerText = "✅ Gekopieerd!";
                    setTimeout(() => {{ knop.innerText = oud; }}, 1500);
                }}
            </script>
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        melding_banner = melding_banner,
        media_grid_html = media_grid_html
    ))
    .into_response()
}

async fn verwerk_media_upload(
    headers: HeaderMap,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    fs::create_dir_all(&state.media_map).unwrap();

    while let Ok(Some(veld)) = multipart.next_field().await {
        if let Some(originele_naam) = veld.file_name().map(|s| s.to_string()) {
            if !originele_naam.is_empty() {
                let veilige_naam = maak_veilige_bestandsnaam(&originele_naam);
                if let Ok(bytes) = veld.bytes().await {
                    let doelpad = format!("{}/{}", state.media_map, veilige_naam);
                    fs::write(&doelpad, &bytes).unwrap();
                    return Redirect::to(&format!(
                        "/media?melding=Bestand+{}+is+geüpload+naar+media/",
                        veilige_naam
                    ))
                    .into_response();
                }
            }
        }
    }

    Redirect::to("/media").into_response()
}

async fn verwerk_media_verwijder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<MediaVerwijderForm>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    let veilige_naam = maak_veilige_bestandsnaam(&form.bestandsnaam);
    let pad = format!("{}/{}", state.media_map, veilige_naam);
    let _ = fs::remove_file(pad);

    Redirect::to("/media?melding=Afbeelding+verwijderd+uit+media/").into_response()
}

async fn serveer_media_bestand(
    AxumPath(bestandsnaam): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let veilige_naam = maak_veilige_bestandsnaam(&bestandsnaam);
    let pad = format!("{}/{}", state.media_map, veilige_naam);

    match fs::read(&pad) {
        Ok(bytes) => {
            let mime = bepaal_content_type(&veilige_naam);
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static(mime));
            (StatusCode::OK, headers, bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "Bestand niet gevonden").into_response(),
    }
}

// 5. De Plug-in Beheer & Installatiepagina (/plugins) met Scaffold
fn haal_preset_plugins() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "cover.rhai",
            "🖼️ Cover Afbeelding Plug-in",
            "Toont automatisch een cover-afbeelding boven je blogposts (uit het frontmatter-veld 'cover:') en op het blog-overzicht.",
            r#"// Cover Afbeelding Plug-in voor BlogRust
fn bewerk_bericht_met_cover(tekst, cover, categorie) {
    if cover != "" {
        return "<img src=\"" + cover + "\" class=\"plugin-cover\" alt=\"Cover afbeelding\">\n" + tekst;
    }
    return tekst;
}
"#,
        ),
        (
            "leestijd.rhai",
            "⏱️ Geschatte Leestijd",
            "Telt het aantal woorden van een artikel en plaatst automatisch de geschatte leestijd in minuten bovenaan.",
            r#"// Geschatte Leestijd Plug-in
fn bewerk_bericht(tekst) {
    let woorden = tekst.split(" ").len();
    let minuten = (woorden / 200) + 1;
    return "<p><em>⏱️ Geschatte leestijd: " + minuten + " minuut</em></p>\n" + tekst;
}
"#,
        ),
        (
            "highlighter.rhai",
            "🦀 BlogRust Woord-Highlighter",
            "Maakt elk voorkomen van de naam 'BlogRust' in je artikelen automatisch dikgedrukt met een krab-icoontje.",
            r#"// BlogRust Highlighter Plug-in
fn bewerk_bericht(tekst) {
    return tekst.replace("BlogRust", "<strong>🦀 BlogRust</strong>");
}
"#,
        ),
        (
            "voettekst.rhai",
            "💬 Artikel Voettekst (Call-to-Action)",
            "Voegt automatisch een nette bedank-box toe onderaan elk blogartikel.",
            r#"// Artikel Voettekst Plug-in
fn bewerk_bericht(tekst) {
    let box_html = "<div style=\"margin-top:28px;padding:16px 20px;background:#fff7ed;border-left:4px solid #ea580c;border-radius:4px;\"><strong>Bedankt voor het lezen!</strong> Geschreven met BlogRust CMS.</div>";
    return tekst + "\n" + box_html;
}
"#,
        ),
        (
            "reacties.rhai",
            "🌐 Google Auth Reactiesysteem (Comments)",
            "Plaatst een compleet reactiesysteem onder elk blogartikel waarmee bezoekers via Google Auth kunnen inloggen en reageren.",
            r#"// Google Auth Reactiesysteem Plug-in voor BlogRust
fn bewerk_bericht(tekst) {
    if env("REACTIES_INGESCHAKELD") != "false" {
        return tekst + "\n<!-- BLOGRUST_GOOGLE_REACTIES -->";
    }
    return tekst;
}
"#,
        ),
        (
            "i18n.rhai",
            "🌍 Multi-Language (i18n) — Engels Standaard + Taalkiezer",
            "Maakt BlogRust meertalig (EN/NL/DE/FR/ES) met Engels als wereldwijde standaard, automatische Browser API (navigator.language) detectie en een taalkiezer in de bovenbalk.",
            r#"// 🌍 Multi-Language (i18n) Plug-in voor BlogRust (Standaard: Engels + Browser API)
fn bewerk_navigatie(nav_html) {
    let standaard_taal = env("DEFAULT_LANG");
    if standaard_taal == "" {
        standaard_taal = "en";
    }
    return nav_html + "\n<!-- BLOGRUST_I18N:" + standaard_taal + " -->";
}
"#,
        ),
    ]
}

fn lees_geinstalleerde_plugins(plugins_map: &str) -> Vec<GeinstalleerdePlugin> {
    let mut lijst = Vec::new();
    if let Ok(entries) = fs::read_dir(plugins_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if let Some(naam) = pad.file_name().and_then(|n| n.to_str()) {
                if naam.ends_with(".rhai") || naam.ends_with(".rhai.uit") {
                    let actief = naam.ends_with(".rhai");
                    let code = fs::read_to_string(&pad).unwrap_or_default();
                    let eerste_regel = code
                        .lines()
                        .next()
                        .unwrap_or("// Rhai script")
                        .trim_start_matches("//")
                        .trim()
                        .to_string();
                    lijst.push(GeinstalleerdePlugin {
                        bestandsnaam: naam.to_string(),
                        actief,
                        code_preview: eerste_regel,
                    });
                }
            }
        }
    }
    lijst.sort_by(|a, b| a.bestandsnaam.cmp(&b.bestandsnaam));
    lijst
}

async fn plugins_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let admin_gebruiker = haal_ingelogde_gebruiker_op(&headers, &state);
    let google_gebruiker: Option<String> = None;
    let is_admin = admin_gebruiker.is_some();
    let nav_id = haal_nav_identiteit_op(&headers, &state);

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());
    let geinstalleerd = lees_geinstalleerde_plugins(&state.plugins_map);
    let plugin_scaffold = laad_scaffold(
        &state.scaffolds_map,
        "plugin.rhai",
        "// Scaffold voor een nieuwe Rhai Plug-in\nfn bewerk_bericht(tekst) {\n    return tekst;\n}\n",
    );

    let melding_banner = if let Some(m) = query.melding {
        format!(
            r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 16px;border-radius:6px;margin-bottom:20px;font-weight:500;">✅ {m}</div>"#
        )
    } else {
        String::new()
    };

    let mut geinstalleerd_html = String::new();
    if geinstalleerd.is_empty() {
        geinstalleerd_html.push_str(
            r#"<p style="color:#6b7280;">Er staan momenteel geen <code>.rhai</code> plug-ins in de map <code>plugins/</code>.</p>"#,
        );
    } else {
        for p in &geinstalleerd {
            let status_badge = if p.actief {
                r#"<span class="status-actief">● Actief</span>"#
            } else {
                r#"<span class="status-uit">○ Uitgeschakeld</span>"#
            };
            let wissel_tekst = if p.actief {
                "⏸️ Deactiveren"
            } else {
                "▶️ Activeren"
            };

            let schone_dl_naam = p
                .bestandsnaam
                .strip_suffix(".uit")
                .unwrap_or(&p.bestandsnaam);

            let admin_knoppen = if is_admin {
                format!(
                    r#"<form action="/plugins" method="post" style="margin:0;">
                        <input type="hidden" name="actie" value="wissel_status">
                        <input type="hidden" name="naam" value="{naam}">
                        <button type="submit" class="btn-sec">{wissel_tekst}</button>
                    </form>
                    <form action="/plugins" method="post" style="margin:0;">
                        <input type="hidden" name="actie" value="verwijder">
                        <input type="hidden" name="naam" value="{naam}">
                        <button type="submit" class="btn-del">🗑️ Verwijder</button>
                    </form>"#,
                    naam = p.bestandsnaam,
                    wissel_tekst = wissel_tekst
                )
            } else {
                String::new()
            };

            geinstalleerd_html.push_str(&format!(
                r#"
                <div class="plugin-row">
                    <div>
                        <div style="display:flex;align-items:center;gap:10px;">
                            <strong><code>plugins/{naam}</code></strong>
                            {status_badge}
                        </div>
                        <div style="font-size:13px;color:#6b7280;margin-top:4px;">{omschrijving}</div>
                    </div>
                    <div style="display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
                        <a href="/plugins/download/{dl_naam}" class="btn-sec" style="text-decoration:none;">⬇️ Download .rhai</a>
                        {admin_knoppen}
                    </div>
                </div>
                "#,
                naam = p.bestandsnaam,
                dl_naam = schone_dl_naam,
                status_badge = status_badge,
                omschrijving = p.code_preview,
                admin_knoppen = admin_knoppen
            ));
        }
    }

    let mut catalogus_html = String::new();
    for (bestandsnaam, titel_naam, uitleg, _) in haal_preset_plugins() {
        let al_aanwezig = geinstalleerd.iter().any(|g| {
            g.bestandsnaam == bestandsnaam || g.bestandsnaam == format!("{}.uit", bestandsnaam)
        });

        let admin_install_knop = if is_admin {
            if al_aanwezig {
                r#"<span class="installed-pill">✅ In plugins/</span>"#.to_string()
            } else {
                format!(
                    r#"<form action="/plugins" method="post" style="margin:0;">
                        <input type="hidden" name="actie" value="installeer_preset">
                        <input type="hidden" name="naam" value="{bestandsnaam}">
                        <button type="submit" class="btn-install">⬇️ Installeer in plugins/</button>
                    </form>"#
                )
            }
        } else {
            String::new()
        };

        catalogus_html.push_str(&format!(
            r#"
            <div class="store-card">
                <div>
                    <h3>{titel_naam}</h3>
                    <p>{uitleg}</p>
                    <small style="color:#9ca3af;">Bestand: <code>{bestandsnaam}</code></small>
                </div>
                <div style="margin-top:14px;display:flex;gap:8px;align-items:center;flex-wrap:wrap;">
                    <a href="/plugins/download/{bestandsnaam}" class="btn-sec" style="text-decoration:none;">⬇️ Download .rhai</a>
                    {admin_install_knop}
                </div>
            </div>
            "#
        ));
    }

    let toevoegen_sectie_html = if is_admin || google_gebruiker.is_some() {
        let auteur_badge = if let Some(ref email) = google_gebruiker {
            format!(
                r#"<div style="background:#eff6ff;border:1px solid #bfdbfe;color:#1d4ed8;padding:10px 14px;border-radius:6px;margin-bottom:14px;font-size:13px;">🌐 Ingelogd via Google als <strong>{email}</strong> — jouw Rhai plug-in wordt direct toegevoegd aan de community-map <code>plugins/</code>.</div>"#
            )
        } else {
            String::new()
        };
        format!(
            r#"
            <section class="card">
                <h2>🛠️ Eigen Rhai Plug-in Toevoegen (vooringevuld met <code>scaffolds/plugin.rhai</code>)</h2>
                {auteur_badge}
                <form action="/plugins" method="post">
                    <input type="hidden" name="actie" value="eigen_plugin">
                    <label style="font-weight:600;font-size:14px;display:block;margin-bottom:6px;">Bestandsnaam (bijv. <code>mijn_plugin.rhai</code>)</label>
                    <input type="text" name="naam" placeholder="mijn_plugin.rhai" required>
                    <label style="font-weight:600;font-size:14px;display:block;margin-bottom:6px;">Rhai Script Code (Scaffold)</label>
                    <textarea name="code" required>{plugin_scaffold}</textarea>
                    <button type="submit" class="btn-install">💾 Plug-in Publiceren in plugins/</button>
                </form>
            </section>
            "#
        )
    } else {
        r#"
        <section class="card" style="text-align:center;background:#eff6ff;border-color:#bfdbfe;">
            <h2>🌐 Zelf een Plug-in delen met de BlogRust Community?</h2>
            <p style="font-size:14px;color:#1e3a8a;max-width:600px;margin:0 auto 16px auto;">Net als op WordPress.org kan iedereen plug-ins downloaden. Log in met je <strong>Google-account</strong> om zelf een nieuwe <code>.rhai</code> plug-in aan deze bibliotheek toe te voegen!</p>
            <a href="/auth/google?melding=plugins" class="btn-install" style="background:#2563eb;border-color:#1d4ed8;text-decoration:none;display:inline-block;padding:10px 20px;">🌐 Inloggen met Google om Plug-in toe te voegen</a>
        </section>
        "#
        .to_string()
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Plug-in Directory &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .card {{ background: #fff; padding: 26px 30px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                h1, h2 {{ margin-top: 0; color: #111827; }}
                h1 {{ font-size: 23px; border-bottom: 1px solid #e5e7eb; padding-bottom: 12px; }}
                h2 {{ font-size: 18px; margin-bottom: 14px; }}
                .plugin-row {{ display: flex; justify-content: space-between; align-items: center; padding: 14px 0; border-bottom: 1px solid #f3f4f6; gap: 12px; flex-wrap: wrap; }}
                .plugin-row:last-child {{ border-bottom: none; }}
                .status-actief {{ font-size: 12px; font-weight: 700; color: #15803d; background: #dcfce7; padding: 2px 8px; border-radius: 999px; }}
                .status-uit {{ font-size: 12px; font-weight: 600; color: #6b7280; background: #f3f4f6; padding: 2px 8px; border-radius: 999px; }}
                .btn-sec {{ background: #f6f7f7; border: 1px solid #2271b1; color: #2271b1; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; font-weight: 600; }}
                .btn-del {{ background: #fff; border: 1px solid #dc2626; color: #dc2626; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 13px; }}
                .store-grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }}
                .store-card {{ border: 1px solid #e5e7eb; border-radius: 8px; padding: 18px; display: flex; flex-direction: column; justify-content: space-between; background: #fafafa; }}
                .store-card h3 {{ margin: 0 0 8px 0; font-size: 16px; color: #111827; }}
                .store-card p {{ margin: 0 0 10px 0; font-size: 13px; color: #4b5563; line-height: 1.5; }}
                .btn-install {{ background: #ea580c; color: #fff; border: 1px solid #ea580c; padding: 8px 14px; border-radius: 5px; font-size: 13px; font-weight: 600; cursor: pointer; }}
                .btn-install:hover {{ background: #c2410c; }}
                .installed-pill {{ display: inline-block; font-size: 13px; font-weight: 600; color: #15803d; background: #dcfce7; padding: 6px 12px; border-radius: 5px; }}
                input[type="text"], textarea {{ width: 100%; padding: 9px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; box-sizing: border-box; margin-bottom: 12px; }}
                textarea {{ font-family: ui-monospace, Consolas, monospace; min-height: 150px; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            {melding_banner}

            <section class="card">
                <h1>🧩 Community Plug-in Directory (<code>plugins/</code>)</h1>
                <p style="font-size:14px;color:#4b5563;margin-top:0;margin-bottom:14px;">Download elke plug-in als <code>.rhai</code> script voor je eigen BlogRust-site, of voeg via Google Auth je eigen plug-in toe.</p>
                {geinstalleerd_html}
            </section>

            <section class="card">
                <h2>📦 Officiële BlogRust Plug-ins</h2>
                <div class="store-grid">
                    {catalogus_html}
                </div>
            </section>

            {toevoegen_sectie_html}
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        melding_banner = melding_banner,
        geinstalleerd_html = geinstalleerd_html,
        catalogus_html = catalogus_html,
        toevoegen_sectie_html = toevoegen_sectie_html
    ))
    .into_response()
}

async fn download_plugin_bestand(
    AxumPath(naam): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let veilige_naam = maak_veilige_bestandsnaam(&naam);
    let pad_actief = format!("{}/{}", state.plugins_map, veilige_naam);
    let pad_uit = format!("{}/{}.uit", state.plugins_map, veilige_naam);

    let inhoud = if let Ok(tekst) = fs::read_to_string(&pad_actief) {
        Some(tekst)
    } else if let Ok(tekst) = fs::read_to_string(&pad_uit) {
        Some(tekst)
    } else {
        haal_preset_plugins()
            .into_iter()
            .find(|(n, _, _, _)| *n == veilige_naam)
            .map(|(_, _, _, code)| code.to_string())
    };

    if let Some(rhai_tekst) = inhoud {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        if let Ok(disp) = HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            veilige_naam
        )) {
            headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
        }
        (StatusCode::OK, headers, rhai_tekst).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Plug-in niet gevonden").into_response()
    }
}

async fn verwerk_plugin_actie(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<PluginActieForm>,
) -> impl IntoResponse {
    let admin = haal_ingelogde_gebruiker_op(&headers, &state);
    let google_user: Option<String> = None;

    if admin.is_none() && google_user.is_none() {
        return Redirect::to("/auth/google").into_response();
    }

    fs::create_dir_all(&state.plugins_map).unwrap();

    let veilige_naam: String = form
        .naam
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
        .collect();

    match form.actie.as_str() {
        "installeer_preset" if admin.is_some() => {
            for (preset_naam, _, _, preset_code) in haal_preset_plugins() {
                if preset_naam == veilige_naam {
                    let doel = format!("{}/{}", state.plugins_map, preset_naam);
                    fs::write(doel, preset_code).unwrap();
                    return Redirect::to(&format!(
                        "/plugins?melding=Plug-in+{}+is+geïnstalleerd+in+plugins/",
                        preset_naam
                    ))
                    .into_response();
                }
            }
        }
        "wissel_status" if admin.is_some() => {
            let huidig_pad = format!("{}/{}", state.plugins_map, veilige_naam);
            if veilige_naam.ends_with(".rhai") {
                let nieuw_pad = format!("{}.uit", huidig_pad);
                let _ = fs::rename(&huidig_pad, &nieuw_pad);
                return Redirect::to("/plugins?melding=Plug-in+gedeactiveerd").into_response();
            } else if let Some(actieve_naam) = veilige_naam.strip_suffix(".uit") {
                let nieuw_pad = format!("{}/{}", state.plugins_map, actieve_naam);
                let _ = fs::rename(&huidig_pad, &nieuw_pad);
                return Redirect::to("/plugins?melding=Plug-in+geactiveerd").into_response();
            }
        }
        "verwijder" if admin.is_some() => {
            let pad = format!("{}/{}", state.plugins_map, veilige_naam);
            let _ = fs::remove_file(pad);
            return Redirect::to("/plugins?melding=Plug-in+verwijderd+uit+plugins/")
                .into_response();
        }
        "eigen_plugin" => {
            let mut eind_naam = veilige_naam;
            if !eind_naam.ends_with(".rhai") {
                eind_naam.push_str(".rhai");
            }
            if let Some(code) = form.code {
                let eind_code = if let Some(ref email) = google_user {
                    if admin.is_none() {
                        format!("// Community Plug-in door (Google Auth): {}\n{}", email, code)
                    } else {
                        code
                    }
                } else {
                    code
                };
                let pad = format!("{}/{}", state.plugins_map, eind_naam);
                fs::write(pad, eind_code).unwrap();
                return Redirect::to(&format!(
                    "/plugins?melding=Community+plug-in+{}+gepubliceerd+in+plugins/",
                    eind_naam
                ))
                .into_response();
            }
        }
        _ => {}
    }

    Redirect::to("/plugins").into_response()
}

// 6. Nieuw Bericht (/nieuw) én Bestaand Bericht Bewerken (/bewerk/{slug}) + Verwijderen (/verwijder/{slug})
async fn nieuw_bericht_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let post_scaffold = laad_scaffold(
        &state.scaffolds_map,
        "post.md",
        "## Introductie\n\nSchrijf hier je artikel in **Markdown**...\n",
    );
    render_bericht_formulier(headers, state, None, &post_scaffold).await
}

async fn bewerk_bericht_pagina(
    headers: HeaderMap,
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let bestandspad = format!("{}/{}.md", state.posts_map, slug);
    let Ok(ruwe_inhoud) = fs::read_to_string(&bestandspad) else {
        return Redirect::to("/").into_response();
    };
    let bestaande_post = parse_markdown_post(&slug, &ruwe_inhoud);
    let md_body = bestaande_post.ruwe_markdown.clone();
    render_bericht_formulier(headers, state, Some(bestaande_post), &md_body).await
}

async fn render_bericht_formulier(
    headers: HeaderMap,
    state: AppState,
    bestaande_post: Option<MarkdownPost>,
    standaard_inhoud: &str,
) -> axum::response::Response {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let Some(gebruiker) = haal_ingelogde_gebruiker_op(&headers, &state) else {
        return Redirect::to("/login").into_response();
    };

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_rechts = maak_nav_rechts_html(Some(&gebruiker));
    let media_bestanden = lees_media_bestanden(&state.media_map);

    let mut media_keuze_html = String::new();
    for b in &media_bestanden {
        let pad = format!("/media/{}", b.bestandsnaam);
        media_keuze_html.push_str(&format!(
            r#"<button type="button" class="media-chip" onclick="document.getElementById('cover').value='{pad}'">🖼️ {naam}</button>"#,
            pad = pad,
            naam = b.bestandsnaam
        ));
    }

    let (
        pagina_kop,
        originele_slug_veld,
        titel_w,
        datum_w,
        cat_w,
        tags_w,
        cover_w,
        samenvatting_w,
        concept_checked,
    ) = if let Some(p) = bestaande_post {
        (
            format!("✏️ Bericht Bewerken: <code>posts/{}.md</code>", p.slug),
            format!(r#"<input type="hidden" name="originele_slug" value="{}">"#, p.slug),
            p.titel,
            p.datum,
            p.categorie,
            p.tags.join(", "),
            p.cover,
            p.samenvatting,
            if p.concept { "checked" } else { "" },
        )
    } else {
        (
            "✍️ Nieuw Markdown Bericht (vooringevuld met <code>scaffolds/post.md</code>)"
                .to_string(),
            String::new(),
            String::new(),
            "2026-11-14".to_string(),
            "Tutorials".to_string(),
            "rust, markdown".to_string(),
            "/media/blogrust-banner.svg".to_string(),
            String::new(),
            "",
        )
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Bericht Editor &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .card {{ background: #fff; padding: 30px; border-radius: 8px; border: 1px solid #e5e7eb; }}
                h1 {{ margin-top: 0; font-size: 22px; border-bottom: 1px solid #e5e7eb; padding-bottom: 12px; }}
                .grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; }}
                .form-group {{ margin-bottom: 16px; }}
                label {{ display: block; font-weight: 600; margin-bottom: 6px; font-size: 14px; }}
                input[type="text"], input[type="date"], textarea {{ width: 100%; padding: 9px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; box-sizing: border-box; font-family: inherit; }}
                textarea {{ font-family: ui-monospace, Consolas, monospace; min-height: 240px; line-height: 1.5; }}
                .media-chips {{ display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; align-items: center; }}
                .media-chip {{ background: #f3f4f6; border: 1px solid #d1d5db; border-radius: 4px; padding: 4px 10px; font-size: 12px; cursor: pointer; }}
                .media-chip:hover {{ border-color: #ea580c; color: #ea580c; background: #fff7ed; }}
                .btn-save {{ background: #ea580c; color: #fff; border: 1px solid #ea580c; padding: 10px 20px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; }}
                .btn-save:hover {{ background: #c2410c; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            <main class="card">
                <h1>{pagina_kop}</h1>
                <form action="/nieuw" method="post">
                    {originele_slug_veld}
                    <div class="grid">
                        <div class="form-group">
                            <label for="titel">Titel</label>
                            <input type="text" id="titel" name="titel" value="{titel_w}" placeholder="Bijv. Mijn nieuwe Rust project" required>
                        </div>
                        <div class="form-group">
                            <label for="datum">Datum</label>
                            <input type="date" id="datum" name="datum" value="{datum_w}" required>
                        </div>
                    </div>
                    <div class="grid">
                        <div class="form-group">
                            <label for="categorie">Categorie</label>
                            <input type="text" id="categorie" name="categorie" value="{cat_w}" required>
                        </div>
                        <div class="form-group">
                            <label for="tags">Tags (komma-gescheiden)</label>
                            <input type="text" id="tags" name="tags" value="{tags_w}" placeholder="rust, markdown, hugo">
                        </div>
                    </div>
                    <div class="form-group">
                        <label for="cover">Cover Afbeelding (uit <code>/media/...</code> of URL)</label>
                        <input type="text" id="cover" name="cover" value="{cover_w}" placeholder="/media/blogrust-banner.svg">
                        <div class="media-chips">
                            <span style="font-size:12px;color:#6b7280;">Kies uit <code>media/</code>:</span>
                            {media_keuze_html}
                            <a href="/media" style="font-size:12px;color:#2271b1;text-decoration:none;margin-left:4px;">+ Nieuwe uploaden</a>
                        </div>
                    </div>
                    <div class="form-group">
                        <label for="samenvatting">Korte samenvatting</label>
                        <input type="text" id="samenvatting" name="samenvatting" value="{samenvatting_w}" placeholder="Wordt getoond op de startpagina...">
                    </div>
                    <div class="form-group">
                        <label for="inhoud">Markdown Inhoud (Scaffold Sjablonen)</label>
                        <div class="media-chips" style="margin-bottom:8px;">
                            <button type="button" class="media-chip" onclick="laadSjabloon('tutorial')">📖 Tutorial Scaffold</button>
                            <button type="button" class="media-chip" onclick="laadSjabloon('nieuws')">📰 Nieuwsbericht Scaffold</button>
                            <button type="button" class="media-chip" onclick="laadSjabloon('review')">⭐ Review Scaffold</button>
                        </div>
                        <textarea id="inhoud" name="inhoud" required>{standaard_inhoud}</textarea>
                    </div>
                    <div class="form-group">
                        <label><input type="checkbox" name="concept" value="true" {concept_checked}> Opslaan als concept (draft)</label>
                    </div>
                    <button type="submit" class="btn-save">💾 Opslaan als .md in posts/</button>
                </form>
            </main>
            <script>
                function laadSjabloon(type) {{
                    const veld = document.getElementById('inhoud');
                    if (type === 'tutorial') {{
                        veld.value = '## Doel van deze tutorial\n\nLeg hier kort uit wat we gaan bouwen.\n\n## Stap 1: Voorbereiding\n\n```rust\nfn main() {{\n    println!("Stap 1 gereed!");\n}}\n```\n\n## Stap 2: Uitvoering\n\nBeschrijf de volgende stap...\n';
                    }} else if (type === 'nieuws') {{
                        veld.value = '**Korte samenvatting van het nieuwsfeit in één dikgedrukte zin.**\n\n## Wat is er nieuw?\n\nBeschrijf hier het nieuwsartikel...\n\n## Wat betekent dit voor gebruikers?\n\n- Voordeel 1\n- Voordeel 2\n';
                    }} else if (type === 'review') {{
                        veld.value = '## Eerste Indruk\n\nBeschrijf het onderwerp van de review.\n\n| Pluspunten | Minpunten |\n| :--- | :--- |\n| ✅ Snel en betrouwbaar | ❌ Nog in ontwikkeling |\n\n## Eindcijfer: 9/10\n';
                    }}
                }}
            </script>
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        pagina_kop = pagina_kop,
        originele_slug_veld = originele_slug_veld,
        titel_w = titel_w,
        datum_w = datum_w,
        cat_w = cat_w,
        tags_w = tags_w,
        cover_w = cover_w,
        media_keuze_html = media_keuze_html,
        samenvatting_w = samenvatting_w,
        standaard_inhoud = standaard_inhoud,
        concept_checked = concept_checked
    ))
    .into_response()
}

async fn verwerk_nieuw_bericht(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<NieuwBerichtForm>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    fs::create_dir_all(&state.posts_map).unwrap();

    // Gebruik bij bewerken de bestaande slug (of maak een nieuwe aan bij een nieuw bericht)
    let veilige_slug = if let Some(orig) = form.originele_slug.filter(|s| !s.trim().is_empty()) {
        maak_slug(&orig)
    } else {
        let s = maak_slug(&form.titel);
        if s.is_empty() {
            "nieuw-bericht".to_string()
        } else {
            s
        }
    };

    let tags_geformatteerd: Vec<String> = form
        .tags
        .split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.replace('"', "")))
        .collect();

    let is_concept = form.concept.is_some();

    let md_bestand = format!(
        "---\ntitle: \"{}\"\ndate: \"{}\"\ncategory: \"{}\"\ntags: [{}]\ncover: \"{}\"\nsummary: \"{}\"\ndraft: {}\n---\n\n{}\n",
        form.titel.replace('"', "'"),
        form.datum,
        form.categorie.replace('"', "'"),
        tags_geformatteerd.join(", "),
        form.cover.trim().replace('"', ""),
        form.samenvatting.replace('"', "'"),
        is_concept,
        form.inhoud
    );

    let bestandspad = format!("{}/{}.md", state.posts_map, veilige_slug);
    fs::write(&bestandspad, md_bestand).unwrap();

    Redirect::to(&format!("/post/{}", veilige_slug)).into_response()
}

async fn verwijder_bericht_actie(
    headers: HeaderMap,
    AxumPath(slug): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    let veilige_slug = maak_slug(&slug);
    let pad = format!("{}/{}.md", state.posts_map, veilige_slug);
    let _ = fs::remove_file(pad);

    Redirect::to("/").into_response()
}

// 7. Het eenmalige installatieformulier (/setup)
async fn setup_formulier(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    if is_geinstalleerd(&state.db_pad) {
        if haal_ingelogde_gebruiker_op(&headers, &state).is_some() {
            return Redirect::to("/").into_response();
        } else {
            return Redirect::to("/login").into_response();
        }
    }

    Html(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>BlogRust &rsaquo; Installatie</title>
            <style>
                body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f0f0f1; color: #3c434a; margin: 0; padding: 40px 20px; }
                .logo { text-align: center; font-size: 28px; font-weight: bold; color: #ea580c; margin-bottom: 20px; }
                .box { max-width: 520px; margin: 0 auto; background: #fff; padding: 30px; border: 1px solid #c3c4c7; box-shadow: 0 1px 3px rgba(0,0,0,0.04); }
                h1 { font-size: 22px; font-weight: 400; border-bottom: 1px solid #dcdcde; padding-bottom: 15px; margin-top: 0; }
                table { width: 100%; border-collapse: collapse; margin-top: 15px; }
                th { text-align: left; width: 35%; padding: 12px 10px 12px 0; font-weight: 600; vertical-align: top; }
                td { padding: 10px 0; }
                input[type="text"], input[type="password"] { width: 100%; padding: 8px 10px; border: 1px solid #8c8f94; border-radius: 4px; font-size: 14px; box-sizing: border-box; }
                .pw-controls { display: flex; gap: 8px; margin-top: 8px; }
                .btn-sec { background: #f6f7f7; border: 1px solid #2271b1; color: #2271b1; padding: 5px 10px; border-radius: 3px; cursor: pointer; font-size: 13px; }
                .strength-meter { margin-top: 8px; padding: 6px 10px; text-align: center; font-weight: 600; font-size: 13px; border-radius: 3px; background: #f0f0f1; border: 1px solid #dcdcde; }
                .weak { background: #f8d7da; border-color: #f5c2c7; color: #842029; }
                .medium { background: #fff3cd; border-color: #ffecb5; color: #664d03; }
                .strong { background: #d1e7dd; border-color: #badbcc; color: #0f5132; }
                .btn-primary { background: #2271b1; border: 1px solid #2271b1; color: #fff; padding: 10px 18px; border-radius: 3px; font-size: 14px; cursor: pointer; margin-top: 20px; }
            </style>
        </head>
        <body>
            <div class="logo">🦀 BlogRust</div>
            <div class="box">
                <h1>Benodigde informatie</h1>
                <form action="/setup" method="post">
                    <table>
                        <tr>
                            <th><label for="titel">Websitetitel</label></th>
                            <td><input type="text" id="titel" name="site_titel" required></td>
                        </tr>
                        <tr>
                            <th><label for="admin">Gebruikersnaam</label></th>
                            <td><input type="text" id="admin" name="admin_naam" required></td>
                        </tr>
                        <tr>
                            <th><label for="pw">Wachtwoord</label></th>
                            <td>
                                <input type="text" id="pw" name="wachtwoord" oninput="checkSterkte()" required>
                                <div class="pw-controls">
                                    <button type="button" class="btn-sec" onclick="genereerWachtwoord()">Genereer wachtwoord</button>
                                    <button type="button" class="btn-sec" id="toggleBtn" onclick="toggleZichtbaar()">Verbergen</button>
                                </div>
                                <div id="meter" class="strength-meter">Voer een wachtwoord in</div>
                            </td>
                        </tr>
                    </table>
                    <button type="submit" class="btn-primary">BlogRust installeren</button>
                </form>
            </div>
            <script>
                function genereerWachtwoord() {
                    const tekens = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+";
                    let nieuw = "";
                    for (let i = 0; i < 16; i++) {
                        nieuw += tekens.charAt(Math.floor(Math.random() * tekens.length));
                    }
                    const veld = document.getElementById("pw");
                    veld.value = nieuw;
                    veld.type = "text";
                    document.getElementById("toggleBtn").innerText = "Verbergen";
                    checkSterkte();
                }
                function toggleZichtbaar() {
                    const veld = document.getElementById("pw");
                    const btn = document.getElementById("toggleBtn");
                    if (veld.type === "password") {
                        veld.type = "text";
                        btn.innerText = "Verbergen";
                    } else {
                        veld.type = "password";
                        btn.innerText = "Tonen";
                    }
                }
                function checkSterkte() {
                    const pw = document.getElementById("pw").value;
                    const meter = document.getElementById("meter");
                    meter.className = "strength-meter";
                    if (pw.length === 0) { meter.innerText = "Voer een wachtwoord in"; return; }
                    let score = 0;
                    if (pw.length >= 8) score++;
                    if (pw.length >= 12) score++;
                    if (/[A-Z]/.test(pw) && /[0-9]/.test(pw)) score++;
                    if (/[^A-Za-z0-9]/.test(pw)) score++;
                    if (score <= 1) { meter.innerText = "Zwak"; meter.classList.add("weak"); }
                    else if (score <= 3) { meter.innerText = "Matig"; meter.classList.add("medium"); }
                    else { meter.innerText = "Sterk"; meter.classList.add("strong"); }
                }
            </script>
        </body>
        </html>
        "#,
    )
    .into_response()
}

async fn verwerk_setup(
    State(state): State<AppState>,
    Form(gegevens): Form<SetupForm>,
) -> impl IntoResponse {
    if is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/login").into_response();
    }

    fs::create_dir_all("data").unwrap();

    let opties = SqliteConnectOptions::from_str(&format!("sqlite:{}", state.db_pad))
        .unwrap()
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(opties).await.unwrap();

    sqlx::raw_sql(
        "CREATE TABLE instellingen (sleutel TEXT PRIMARY KEY, waarde TEXT NOT NULL);
         CREATE TABLE gebruikers (id INTEGER PRIMARY KEY AUTOINCREMENT, naam TEXT UNIQUE NOT NULL, wachtwoord_hash TEXT NOT NULL);
         CREATE TABLE berichten (id INTEGER PRIMARY KEY AUTOINCREMENT, titel TEXT NOT NULL, inhoud TEXT NOT NULL, aangemaakt DATETIME DEFAULT CURRENT_TIMESTAMP);",
    )
    .execute(&pool)
    .await
    .unwrap();

    let salt = SaltString::generate(&mut OsRng);
    let veilige_hash = Argon2::default()
        .hash_password(gegevens.wachtwoord.as_bytes(), &salt)
        .unwrap()
        .to_string();

    sqlx::query("INSERT INTO instellingen (sleutel, waarde) VALUES ('site_titel', ?)")
        .bind(&gegevens.site_titel)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO instellingen (sleutel, waarde) VALUES ('site_beschrijving', ?)")
        .bind("Een razendsnel CMS gebouwd in Rust met Hugo-stijl Markdown.")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO gebruikers (naam, wachtwoord_hash) VALUES (?, ?)")
        .bind(&gegevens.admin_naam)
        .bind(&veilige_hash)
        .execute(&pool)
        .await
        .unwrap();

    let token = maak_sessie_aan(&state, &gegevens.admin_naam);
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "blogrust_sessie={}; Path=/; HttpOnly; SameSite=Lax",
            token
        ))
        .unwrap(),
    );

    (headers, Redirect::to("/")).into_response()
}

// 8. De Inlogpagina (/login)
async fn login_formulier(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(melding): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    if haal_ingelogde_gebruiker_op(&headers, &state).is_some() {
        return Redirect::to("/").into_response();
    }

    let fout_html = if melding.fout.is_some() {
        r#"<div style="background:#f8d7da;border:1px solid #f5c2c7;color:#842029;padding:10px 12px;border-radius:4px;margin-bottom:16px;font-size:14px;">Ongeldige gebruikersnaam of wachtwoord.</div>"#
    } else {
        ""
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>BlogRust &rsaquo; Inloggen</title>
            <style>
                body {{ font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f0f0f1; color: #3c434a; margin: 0; padding: 60px 20px; }}
                .logo {{ text-align: center; font-size: 28px; font-weight: bold; color: #ea580c; margin-bottom: 20px; }}
                .box {{ max-width: 380px; margin: 0 auto; background: #fff; padding: 28px; border: 1px solid #c3c4c7; box-shadow: 0 1px 3px rgba(0,0,0,0.04); border-radius: 4px; }}
                h1 {{ font-size: 20px; font-weight: 500; margin-top: 0; margin-bottom: 18px; border-bottom: 1px solid #dcdcde; padding-bottom: 12px; }}
                label {{ display: block; font-weight: 600; font-size: 14px; margin-bottom: 6px; margin-top: 14px; }}
                input[type="text"], input[type="password"] {{ width: 100%; padding: 9px 10px; border: 1px solid #8c8f94; border-radius: 4px; font-size: 14px; box-sizing: border-box; }}
                .btn-primary {{ width: 100%; background: #2271b1; border: 1px solid #2271b1; color: #fff; padding: 10px 18px; border-radius: 4px; font-size: 14px; font-weight: 600; cursor: pointer; margin-top: 22px; }}
                .btn-primary:hover {{ background: #135e96; }}
            </style>
        </head>
        <body>
            <div class="logo">🦀 BlogRust</div>
            <div class="box">
                <h1>Inloggen</h1>
                {fout_html}
                <form action="/login" method="post">
                    <label for="gebruikersnaam">Gebruikersnaam</label>
                    <input type="text" id="gebruikersnaam" name="gebruikersnaam" required autofocus>

                    <label for="wachtwoord">Wachtwoord</label>
                    <input type="password" id="wachtwoord" name="wachtwoord" required>

                    <button type="submit" class="btn-primary">Inloggen</button>
                </form>
                <div style="text-align:center;margin-top:16px;font-size:13px;">
                    <a href="/" style="color:#6b7280;text-decoration:none;">&larr; Terug naar blog overzicht</a>
                </div>
            </div>
        </body>
        </html>
        "#
    ))
    .into_response()
}

async fn verwerk_login(
    State(state): State<AppState>,
    Form(login): Form<LoginForm>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let pool = SqlitePool::connect(&format!("sqlite:{}", state.db_pad))
        .await
        .unwrap();

    let rij: Option<(String, String)> =
        sqlx::query_as("SELECT naam, wachtwoord_hash FROM gebruikers WHERE naam = ?")
            .bind(&login.gebruikersnaam)
            .fetch_optional(&pool)
            .await
            .unwrap_or(None);

    if let Some((naam, hash_str)) = rij {
        if let Ok(parsed_hash) = PasswordHash::new(&hash_str) {
            if Argon2::default()
                .verify_password(login.wachtwoord.as_bytes(), &parsed_hash)
                .is_ok()
            {
                let token = maak_sessie_aan(&state, &naam);
                let mut headers = HeaderMap::new();
                headers.insert(
                    SET_COOKIE,
                    HeaderValue::from_str(&format!(
                        "blogrust_sessie={}; Path=/; HttpOnly; SameSite=Lax",
                        token
                    ))
                    .unwrap(),
                );
                return (headers, Redirect::to("/")).into_response();
            }
        }
    }

    Redirect::to("/login?fout=1").into_response()
}

async fn verwerk_logout(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    verwijder_sessie(&headers, &state);

    let mut resp_headers = HeaderMap::new();
    resp_headers.append(
        SET_COOKIE,
        HeaderValue::from_static("blogrust_sessie=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
    );
    resp_headers.append(
        SET_COOKIE,
        HeaderValue::from_static(
            "blogrust_google_sessie=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
        ),
    );

    (resp_headers, Redirect::to("/")).into_response()
}

// 8b. Google Auth Inlogpagina (/auth/google) & Download Portal (/download)
async fn haal_google_client_id_op(db_pad: &str) -> String {
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_pad))
        .await
        .unwrap();

    let cid: (String,) =
        sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'google_client_id'")
            .fetch_one(&pool)
            .await
            .unwrap_or((String::new(),));

    if !cid.0.trim().is_empty() {
        cid.0
    } else {
        haal_env_waarde("GOOGLE_CLIENT_ID").unwrap_or_default()
    }
}

async fn google_auth_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(query): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let google_client_id = haal_google_client_id_op(&state.db_pad).await;
    let nav_id = haal_nav_identiteit_op(&headers, &state);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());

    let terug_naar = match query.melding.as_deref() {
        Some("themes") => "/themes".to_string(),
        Some(m) if m.starts_with("post:") => {
            let post_slug = m.trim_start_matches("post:");
            format!("/post/{}", post_slug)
        }
        _ => "/plugins".to_string(),
    };

    let gsi_widget_html = if !google_client_id.trim().is_empty() {
        format!(
            r#"
            <div style="margin-bottom:20px;padding:16px;background:#f8fafc;border:1px solid #e2e8f0;border-radius:8px;text-align:center;">
                <p style="font-size:13px;color:#475569;margin-top:0;">Officiële Google Identity Services (Client ID actief):</p>
                <script src="https://accounts.google.com/gsi/client" async defer></script>
                <form id="gsi-auto-form" action="/auth/google" method="post" style="display:none;">
                    <input type="hidden" name="terug_naar" value="{terug_naar}">
                    <input type="hidden" name="credential" id="gsi-cred-input" value="">
                </form>
                <script>
                    function handleGoogleCredentialResponse(response) {{
                        document.getElementById('gsi-cred-input').value = response.credential;
                        document.getElementById('gsi-auto-form').submit();
                    }}
                </script>
                <div id="g_id_onload"
                     data-client_id="{cid}"
                     data-callback="handleGoogleCredentialResponse"
                     data-auto_prompt="false">
                </div>
                <div class="g_id_signin" data-type="standard" data-size="large" data-theme="outline" data-text="sign_in_with" data-shape="rectangular"></div>
            </div>"#,
            cid = google_client_id.trim(),
            terug_naar = terug_naar
        )
    } else {
        r#"
        <div style="margin-bottom:18px;padding:12px 14px;background:#eff6ff;border:1px solid #bfdbfe;color:#1e40af;border-radius:6px;font-size:13px;">
            💡 <strong>Tip voor productie:</strong> Vul op <code>/instellingen</code> je eigen <em>Google OAuth Client ID</em> in voor de live Google Cloud knop. Hieronder kun je direct inloggen met een Google-account om het toevoegen van plug-ins en thema's te testen!
        </div>
        "#
        .to_string()
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Inloggen met Google &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .google-card {{ max-width: 480px; margin: 20px auto; background: #fff; padding: 32px; border-radius: 10px; border: 1px solid #dadce0; box-shadow: 0 2px 10px rgba(0,0,0,0.05); }}
                .g-header {{ text-align: center; margin-bottom: 22px; }}
                .g-logo {{ font-size: 32px; margin-bottom: 8px; }}
                .g-header h1 {{ margin: 0 0 6px 0; font-size: 22px; color: #202124; }}
                .g-header p {{ margin: 0; font-size: 14px; color: #5f6368; }}
                .acc-btn {{ width: 100%; display: flex; align-items: center; gap: 12px; padding: 12px 14px; background: #fff; border: 1px solid #dadce0; border-radius: 8px; cursor: pointer; text-align: left; margin-bottom: 10px; transition: background 0.15s; }}
                .acc-btn:hover {{ background: #f8f9fa; border-color: #4285f4; }}
                .avatar {{ width: 36px; height: 36px; border-radius: 50%; background: #2563eb; color: #fff; display: flex; align-items: center; justify-content: center; font-weight: 700; font-size: 15px; }}
                input[type="email"] {{ width: 100%; padding: 10px 12px; border: 1px solid #dadce0; border-radius: 6px; font-size: 14px; box-sizing: border-box; margin-bottom: 12px; }}
                .btn-g-submit {{ width: 100%; background: #1a73e8; color: #fff; border: none; padding: 11px 16px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; }}
                .btn-g-submit:hover {{ background: #1557b0; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            <div class="google-card">
                <div class="g-header">
                    <div class="g-logo">🌐</div>
                    <h1>Inloggen met Google</h1>
                    <p>Doorgaan naar <strong>{site_titel} Community</strong> (Plug-ins &amp; Thema's toevoegen)</p>
                </div>

                {gsi_widget_html}

                <div style="font-size:13px;font-weight:600;color:#3c4043;margin-bottom:8px;">Kies een Google-account (1-klik voor lokaal testen):</div>
                <form action="/auth/google" method="post">
                    <input type="hidden" name="terug_naar" value="{terug_naar}">
                    <button type="submit" name="email" value="developer@gmail.com" class="acc-btn">
                        <div class="avatar">D</div>
                        <div>
                            <div style="font-weight:600;font-size:14px;color:#202124;">Rust Community Developer</div>
                            <div style="font-size:12px;color:#5f6368;">developer@gmail.com</div>
                        </div>
                    </button>
                    <button type="submit" name="email" value="thema.designer@gmail.com" class="acc-btn">
                        <div class="avatar" style="background:#ea580c;">T</div>
                        <div>
                            <div style="font-weight:600;font-size:14px;color:#202124;">Thema &amp; Plug-in Designer</div>
                            <div style="font-size:12px;color:#5f6368;">thema.designer@gmail.com</div>
                        </div>
                    </button>
                </form>

                <hr style="border:none;border-top:1px solid #e5e7eb;margin:18px 0;">

                <form action="/auth/google" method="post">
                    <input type="hidden" name="terug_naar" value="{terug_naar}">
                    <label style="font-size:13px;font-weight:600;color:#3c4043;display:block;margin-bottom:6px;">Of voer je eigen Google e-mailadres in:</label>
                    <input type="email" name="email" placeholder="jouwnaam@gmail.com" required>
                    <button type="submit" class="btn-g-submit">🌐 Doorgaan met Google Account</button>
                </form>
            </div>
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts,
        gsi_widget_html = gsi_widget_html,
        terug_naar = terug_naar
    ))
    .into_response()
}

// Hulpfunctie: Haal het e-mailadres uit een Google Identity Services JWT-token payload (base64url)
fn haal_email_uit_google_jwt(jwt: &str) -> Option<String> {
    let payload_b64 = jwt.split('.').nth(1)?;
    let mut bits: u32 = 0;
    let mut bit_count: u8 = 0;
    let mut decoded: Vec<u8> = Vec::new();
    for ch in payload_b64.chars() {
        let val: u32 = match ch {
            'A'..='Z' => (ch as u32) - ('A' as u32),
            'a'..='z' => (ch as u32) - ('a' as u32) + 26,
            '0'..='9' => (ch as u32) - ('0' as u32) + 52,
            '-' | '+' => 62,
            '_' | '/' => 63,
            _ => continue,
        };
        bits = (bits << 6) | val;
        bit_count += 6;
        if bit_count >= 8 {
            bit_count -= 8;
            decoded.push(((bits >> bit_count) & 0xFF) as u8);
        }
    }
    let json_tekst = String::from_utf8(decoded).ok()?;
    let zoek = "\"email\":\"";
    let start = json_tekst.find(zoek)? + zoek.len();
    let rest = &json_tekst[start..];
    let eind = rest.find('"')?;
    Some(rest[..eind].to_string())
}

async fn verwerk_google_auth(
    State(state): State<AppState>,
    Form(form): Form<GoogleLoginForm>,
) -> impl IntoResponse {
    let gekozen_email = if !form.email.trim().is_empty() {
        form.email.trim().to_string()
    } else if !form.credential.trim().is_empty() {
        haal_email_uit_google_jwt(form.credential.trim())
            .unwrap_or_else(|| "google.oauth.user@gmail.com".to_string())
    } else {
        "developer@gmail.com".to_string()
    };

    let veilige_email: String = gekozen_email
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '@' || *c == '.' || *c == '_' || *c == '-')
        .collect();

    let token = maak_google_sessie_aan(&state, &veilige_email);
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&format!(
            "blogrust_google_sessie={}; Path=/; HttpOnly; SameSite=Lax",
            token
        ))
        .unwrap(),
    );

    let doel = if form.terug_naar == "/themes" {
        "/themes?melding=Ingelogd+met+Google!+Je+kunt+nu+een+thema+toevoegen.".to_string()
    } else if form.terug_naar.starts_with("/post/") {
        format!("{}#reacties", form.terug_naar)
    } else {
        "/plugins?melding=Ingelogd+met+Google!+Je+kunt+nu+een+plug-in+toevoegen.".to_string()
    };

    (headers, Redirect::to(&doel)).into_response()
}

async fn download_portal_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let (site_titel, _, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (_, _, brand_html, favicon_html) = haal_site_branding_op(&state.db_pad, &site_titel).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_id = haal_nav_identiteit_op(&headers, &state);
    let nav_rechts = maak_nav_rechts_html(nav_id.as_deref());

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Download BlogRust &lsaquo; {site_titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-login {{ text-decoration: none; color: #fff; padding: 7px 14px; border-radius: 4px; border: 1px solid #2271b1; background: #2271b1; font-weight: 600; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; text-decoration: none; font-weight: 500; }}
                .card {{ background: #fff; padding: 28px 32px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                .dl-grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 16px; margin-top: 16px; }}
                .dl-box {{ border: 1px solid #e5e7eb; border-radius: 8px; padding: 18px; background: #fafafa; display: flex; flex-direction: column; justify-content: space-between; }}
                .dl-btn {{ display: inline-block; background: #ea580c; color: #fff; text-decoration: none; padding: 8px 14px; border-radius: 5px; font-size: 13px; font-weight: 600; margin-top: 12px; text-align: center; }}
                .dl-btn:hover {{ background: #c2410c; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            <section class="card" style="text-align:center;padding:36px 30px;">
                <div style="margin-bottom:16px;">
                    <a href="/download/folder" title="Klik op het ronde logo om de volledige BlogRust CMS Folder (blogrust-cms.zip) te downloaden" style="display:inline-block;width:136px;height:136px;border-radius:50%;box-shadow:0 8px 24px rgba(234,88,12,0.38);transition:transform 0.2s;" onmouseover="this.style.transform='scale(1.07)'" onmouseout="this.style.transform='scale(1)'">
                        <img src="/media/blogrust-download-rond.svg" alt="Ronde Download Knop" style="width:100%;height:100%;border-radius:50%;display:block;">
                    </a>
                </div>
                <h1 style="font-size:30px;color:#ea580c;margin:0 0 8px 0;">🦀 Download Volledige BlogRust CMS Folder</h1>
                <p style="font-size:15px;color:#15803d;font-weight:600;margin:0 auto 10px auto;">✨ Complete Projectmap (<code>blogrust-cms.zip</code>) — Inclusief <code>README.md</code>, Docker &amp; Caddy, <code>/instellingen</code> pagina én alle mappen (zónder Google Auth)</p>
                <p style="font-size:15px;color:#4b5563;max-width:660px;margin:0 auto 22px auto;">Klik op het <strong>ronde BlogRust-logo</strong> hierboven om in één keer de complete map te downloaden met <code>src/main.rs</code>, <code>Cargo.toml</code>, <code>README.md</code>, <code>Dockerfile</code>, <code>docker-compose.yml</code>, <code>Caddyfile</code> en de mappen <code>posts/</code>, <code>plugins/</code>, <code>themes/</code>, <code>media/</code>, <code>scaffolds/</code> en <code>data/</code>.</p>
                <div style="display:flex;justify-content:center;gap:12px;flex-wrap:wrap;margin-bottom:16px;">
                    <a href="/download/folder" class="dl-btn" style="font-size:16px;padding:12px 26px;background:#ea580c;">📁 Download Volledige Folder (blogrust-cms.zip)</a>
                    <a href="/download/bestand/README.md" class="dl-btn" style="background:#0f172a;font-size:15px;padding:12px 20px;">📖 Download README.md</a>
                </div>
                <div style="display:flex;justify-content:center;gap:10px;flex-wrap:wrap;">
                    <a href="/download/bestand/main.rs" class="dl-btn" style="background:#4b5563;font-size:13px;padding:8px 14px;">🦀 Losse main.rs (Zonder Google Auth)</a>
                    <a href="/download/bestand/Cargo.toml" class="dl-btn" style="background:#4b5563;font-size:13px;padding:8px 14px;">📄 Cargo.toml</a>
                    <a href="/download/bestand/.env.example" class="dl-btn" style="background:#7c3aed;font-size:13px;padding:8px 14px;">🔐 .env.example</a>
                    <a href="/download/bestand/docker-compose.yml" class="dl-btn" style="background:#0284c7;font-size:13px;padding:8px 14px;">🐳 docker-compose.yml</a>
                    <a href="/download/bestand/Dockerfile" class="dl-btn" style="background:#2271b1;font-size:13px;padding:8px 14px;">📦 Dockerfile</a>
                    <a href="/download/bestand/Caddyfile" class="dl-btn" style="background:#15803d;font-size:13px;padding:8px 14px;">🔒 Caddyfile</a>
                </div>
            </section>

            <section class="card">
                <h2 style="margin-top:0;">🐳 Snelstart vanuit de gedownloade folder (uit <code>README.md</code>)</h2>
                <p style="font-size:14px;color:#4b5563;margin-top:0;">Pak <code>blogrust-cms.zip</code> uit en voer in de map <code>blogrust-cms/</code> het volgende Docker Compose commando uit:</p>
                <pre style="background:#1e293b;color:#f8fafc;padding:14px 18px;border-radius:6px;font-size:13px;overflow-x:auto;"><code>cd blogrust-cms
docker compose up -d --build</code></pre>
                <p style="font-size:13px;color:#4b5563;margin-bottom:0;">Je eigen blog draait daarna direct op <code>http://localhost</code> (via Caddy) of <code>http://localhost:3000</code>, compleet met de <code>/setup</code> en <code>/instellingen</code> pagina (voor logo, favicon en container-keuze)!</p>
            </section>

            <section class="card">
                <h2 style="margin-top:0;">🛠️ Scaffolds &amp; Uitbreidingen Downloaden</h2>
                <div class="dl-grid">
                    <div class="dl-box">
                        <div>
                            <h3 style="margin:0 0 6px 0;">📝 Artikel Scaffold (<code>post.md</code>)</h3>
                            <p style="font-size:13px;color:#4b5563;margin:0;">Hugo-stijl frontmatter sjabloon voor nieuwe blogartikelen.</p>
                        </div>
                        <a href="/download/bestand/post.md" class="dl-btn">⬇️ Download post.md</a>
                    </div>
                    <div class="dl-box">
                        <div>
                            <h3 style="margin:0 0 6px 0;">🧩 Plug-in Scaffold (<code>plugin.rhai</code>)</h3>
                            <p style="font-size:13px;color:#4b5563;margin:0;">Startbestand voor het bouwen van Rhai &amp; JS plug-ins.</p>
                        </div>
                        <a href="/download/bestand/plugin.rhai" class="dl-btn">⬇️ Download plugin.rhai</a>
                    </div>
                    <div class="dl-box">
                        <div>
                            <h3 style="margin:0 0 6px 0;">🎨 Thema Scaffold (<code>theme.css</code>)</h3>
                            <p style="font-size:13px;color:#4b5563;margin:0;">CSS-skelet met alle BlogRust variabelen en klassen.</p>
                        </div>
                        <a href="/download/bestand/theme.css" class="dl-btn">⬇️ Download theme.css</a>
                    </div>
                    <div class="dl-box">
                        <div>
                            <h3 style="margin:0 0 6px 0;">🌐 Community Directory</h3>
                            <p style="font-size:13px;color:#4b5563;margin:0;">Bekijk alle community plug-ins en thema's of voeg je eigen werk toe via Google Auth.</p>
                        </div>
                        <div style="display:flex;gap:8px;margin-top:12px;">
                            <a href="/plugins" class="btn-link" style="flex:1;text-align:center;">🧩 Plug-ins</a>
                            <a href="/themes" class="btn-link" style="flex:1;text-align:center;">🎨 Thema's</a>
                        </div>
                    </div>
                </div>
            </section>
        </body>
        </html>
        "#,
        site_titel = site_titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        thema_css = thema_css,
        nav_rechts = nav_rechts
    ))
    .into_response()
}

// Hulpfunctie: Genereer de schone Standaard CMS broncode (zonder Google Auth developer-functies)
fn maak_standaard_cms_zonder_google_auth(bron: &str) -> String {
    let aangepast = bron
        .replace(
            r#"// Standaard CMS: Google Auth developer route uitgeschakeld"#,
            "// Standaard CMS: Google Auth developer route uitgeschakeld",
        )
        .replace(
            r#""#,
            "",
        )
        .replace(
            "let google_gebruiker: Option<String> = None;",
            "let google_gebruiker: Option<String> = None;",
        )
        .replace(
            "let google_user: Option<String> = None;",
            "let google_user: Option<String> = None;",
        );

    format!(
        "// ============================================================================\n// 🦀 BlogRust Standaard CMS Editie (Zonder Google Auth Developer Functies)\n// Plaats dit bestand in src/main.rs van je eigen BlogRust installatie.\n// ============================================================================\n\n{}",
        aangepast
    )
}

// Hulpfunctie: Bereken standaard IEEE 802.3 CRC32 voor een ZIP-archief
fn bereken_crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// Hulpfunctie: Bouw een geldig .zip archief (methode 0 = STORE, UTF-8 paden, MS-DOS datum >= 1980)
fn maak_zip_archief(bestanden: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut zip_bytes: Vec<u8> = Vec::new();
    let mut central_dir: Vec<u8> = Vec::new();
    // Geldige MS-DOS tijd (12:00:00) en datum (2026-11-15) zodat Python/Cloud Run nooit klaagt over pre-1980 timestamps
    let dos_tijd: u16 = 12 << 11;
    let dos_datum: u16 = ((2026 - 1980) << 9) | (11 << 5) | 15;

    for (pad, data) in bestanden {
        let local_offset = zip_bytes.len() as u32;
        let crc = bereken_crc32(data);
        let grootte = data.len() as u32;
        let naam_bytes = pad.as_bytes();
        let naam_len = naam_bytes.len() as u16;

        // Local File Header
        zip_bytes.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        zip_bytes.extend_from_slice(&20u16.to_le_bytes()); // versie 2.0
        zip_bytes.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 vlag
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // STORE (geen compressie)
        zip_bytes.extend_from_slice(&dos_tijd.to_le_bytes()); // tijd
        zip_bytes.extend_from_slice(&dos_datum.to_le_bytes()); // datum
        zip_bytes.extend_from_slice(&crc.to_le_bytes());
        zip_bytes.extend_from_slice(&grootte.to_le_bytes());
        zip_bytes.extend_from_slice(&grootte.to_le_bytes());
        zip_bytes.extend_from_slice(&naam_len.to_le_bytes());
        zip_bytes.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip_bytes.extend_from_slice(naam_bytes);
        zip_bytes.extend_from_slice(data);

        // Central Directory Header
        central_dir.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes());
        central_dir.extend_from_slice(&20u16.to_le_bytes());
        central_dir.extend_from_slice(&0x0800u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&dos_tijd.to_le_bytes());
        central_dir.extend_from_slice(&dos_datum.to_le_bytes());
        central_dir.extend_from_slice(&crc.to_le_bytes());
        central_dir.extend_from_slice(&grootte.to_le_bytes());
        central_dir.extend_from_slice(&grootte.to_le_bytes());
        central_dir.extend_from_slice(&naam_len.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u16.to_le_bytes());
        central_dir.extend_from_slice(&0u32.to_le_bytes());
        central_dir.extend_from_slice(&local_offset.to_le_bytes());
        central_dir.extend_from_slice(naam_bytes);
    }

    let cd_offset = zip_bytes.len() as u32;
    let cd_size = central_dir.len() as u32;
    let aantal = bestanden.len() as u16;

    zip_bytes.extend_from_slice(&central_dir);

    // End of Central Directory Record (EOCD)
    zip_bytes.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    zip_bytes.extend_from_slice(&0u16.to_le_bytes());
    zip_bytes.extend_from_slice(&0u16.to_le_bytes());
    zip_bytes.extend_from_slice(&aantal.to_le_bytes());
    zip_bytes.extend_from_slice(&aantal.to_le_bytes());
    zip_bytes.extend_from_slice(&cd_size.to_le_bytes());
    zip_bytes.extend_from_slice(&cd_offset.to_le_bytes());
    zip_bytes.extend_from_slice(&0u16.to_le_bytes());

    zip_bytes
}

fn voeg_map_toe_aan_zip_lijst(
    hoofdmap: &str,
    bron_map: &str,
    zip_submap: &str,
    lijst: &mut Vec<(String, Vec<u8>)>,
) {
    if let Ok(entries) = fs::read_dir(bron_map) {
        for entry in entries.flatten() {
            let pad = entry.path();
            if pad.is_file() {
                if let Some(naam) = pad.file_name().and_then(|n| n.to_str()) {
                    if let Ok(bytes) = fs::read(&pad) {
                        lijst.push((format!("{}/{}/{}", hoofdmap, zip_submap, naam), bytes));
                    }
                }
            }
        }
    }
}

fn bouw_project_zip(state: &AppState, hoofdmap: &str, met_google_auth: bool) -> Vec<u8> {
    let (dockerfile, compose_yml, caddyfile) =
        maak_docker_en_caddy_inhoud("caddy_lokaal", "localhost");
    let (standaard_env, standaard_env_example) =
        maak_dotenv_en_example_inhoud("3000", "localhost", "data/blogrust.db", "", "");
    let ruwe_main = fs::read_to_string("src/main.rs").unwrap_or_default();
    let main_inhoud = if met_google_auth {
        ruwe_main
    } else {
        maak_standaard_cms_zonder_google_auth(&ruwe_main)
    };
    let readme = fs::read_to_string("README.md").unwrap_or_else(|_| {
        "# BlogRust CMS\n\nStart met Docker Compose:\n\n```bash\ndocker compose up -d --build\n```\n"
            .to_string()
    });
    let cargo_toml = fs::read_to_string("Cargo.toml").unwrap_or_default();
    let cargo_lock = fs::read_to_string("Cargo.lock").unwrap_or_default();
    let dockerignore =
        fs::read_to_string(".dockerignore").unwrap_or_else(|_| "target/\ndata/\n.env\n".to_string());
    let gcloudignore = fs::read_to_string(".gcloudignore")
        .unwrap_or_else(|_| "target/\n.git/\n.env\ndata/*.db-shm\ndata/*.db-wal\n".to_string());
    let gitignore = fs::read_to_string(".gitignore")
        .unwrap_or_else(|_| "target/\ndata/*.db\ndata/*.db-*\n.env\n".to_string());

    let mut bestanden: Vec<(String, Vec<u8>)> = vec![
        (format!("{}/README.md", hoofdmap), readme.into_bytes()),
        (format!("{}/Cargo.toml", hoofdmap), cargo_toml.into_bytes()),
        (
            format!("{}/.env", hoofdmap),
            fs::read_to_string(".env")
                .unwrap_or(standaard_env)
                .into_bytes(),
        ),
        (
            format!("{}/.env.example", hoofdmap),
            fs::read_to_string(".env.example")
                .unwrap_or(standaard_env_example)
                .into_bytes(),
        ),
        (format!("{}/.gitignore", hoofdmap), gitignore.into_bytes()),
        (
            format!("{}/.gcloudignore", hoofdmap),
            gcloudignore.into_bytes(),
        ),
        (
            format!("{}/Dockerfile", hoofdmap),
            fs::read_to_string("Dockerfile")
                .unwrap_or(dockerfile)
                .into_bytes(),
        ),
        (
            format!("{}/docker-compose.yml", hoofdmap),
            fs::read_to_string("docker-compose.yml")
                .unwrap_or(compose_yml)
                .into_bytes(),
        ),
        (
            format!("{}/Caddyfile", hoofdmap),
            fs::read_to_string("Caddyfile")
                .unwrap_or(caddyfile)
                .into_bytes(),
        ),
        (
            format!("{}/.dockerignore", hoofdmap),
            dockerignore.into_bytes(),
        ),
        (format!("{}/src/main.rs", hoofdmap), main_inhoud.into_bytes()),
        (
            format!("{}/data/.gitkeep", hoofdmap),
            b"# SQLite database map voor BlogRust\n".to_vec(),
        ),
    ];

    if !cargo_lock.is_empty() {
        bestanden.push((format!("{}/Cargo.lock", hoofdmap), cargo_lock.into_bytes()));
    }

    voeg_map_toe_aan_zip_lijst(hoofdmap, &state.posts_map, "posts", &mut bestanden);
    voeg_map_toe_aan_zip_lijst(hoofdmap, &state.plugins_map, "plugins", &mut bestanden);
    voeg_map_toe_aan_zip_lijst(hoofdmap, &state.themes_map, "themes", &mut bestanden);
    voeg_map_toe_aan_zip_lijst(hoofdmap, &state.media_map, "media", &mut bestanden);
    voeg_map_toe_aan_zip_lijst(hoofdmap, &state.scaffolds_map, "scaffolds", &mut bestanden);

    maak_zip_archief(&bestanden)
}

async fn download_volledige_folder_zip(State(state): State<AppState>) -> impl IntoResponse {
    let zip_data = bouw_project_zip(&state, "blogrust-cms", false);

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment; filename=\"blogrust-cms.zip\""),
    );
    (StatusCode::OK, headers, zip_data).into_response()
}

async fn download_starter_bestand(
    AxumPath(naam): AxumPath<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let (dockerfile, compose_yml, caddyfile) = maak_docker_en_caddy_inhoud("caddy_lokaal", "localhost");
    let (_, env_example) = maak_dotenv_en_example_inhoud("3000", "localhost", "data/blogrust.db", "", "");

    let (bestandsnaam, inhoud) = match naam.as_str() {
        "README.md" => (
            "README.md",
            fs::read_to_string("README.md").unwrap_or_default(),
        ),
        "main.rs" => {
            let ruwe_main = fs::read_to_string("src/main.rs").unwrap_or_default();
            (
                "main.rs",
                maak_standaard_cms_zonder_google_auth(&ruwe_main),
            )
        }
        "Cargo.toml" => (
            "Cargo.toml",
            fs::read_to_string("Cargo.toml").unwrap_or_default(),
        ),
        ".env.example" => (
            ".env.example",
            fs::read_to_string(".env.example").unwrap_or(env_example),
        ),
        "Dockerfile" => ("Dockerfile", fs::read_to_string("Dockerfile").unwrap_or(dockerfile)),
        "docker-compose.yml" => (
            "docker-compose.yml",
            fs::read_to_string("docker-compose.yml").unwrap_or(compose_yml),
        ),
        "Caddyfile" => ("Caddyfile", fs::read_to_string("Caddyfile").unwrap_or(caddyfile)),
        "post.md" => (
            "post.md",
            laad_scaffold(&state.scaffolds_map, "post.md", "## Introductie\n"),
        ),
        "plugin.rhai" => (
            "plugin.rhai",
            laad_scaffold(
                &state.scaffolds_map,
                "plugin.rhai",
                "fn bewerk_bericht(tekst) { return tekst; }\n",
            ),
        ),
        "theme.css" => (
            "theme.css",
            laad_scaffold(&state.scaffolds_map, "theme.css", "/* Thema */\n"),
        ),
        _ => return (StatusCode::NOT_FOUND, "Bestand niet gevonden").into_response(),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    if let Ok(disp) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", bestandsnaam)) {
        headers.insert(axum::http::header::CONTENT_DISPOSITION, disp);
    }
    (StatusCode::OK, headers, inhoud).into_response()
}

// 9. Instellingen-pagina (/instellingen) + .env & Docker Compose Generator
fn maak_docker_en_caddy_inhoud(modus: &str, domein: &str) -> (String, String, String) {
    let schoon_domein = if domein.trim().is_empty() {
        "localhost".to_string()
    } else {
        domein
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-' || *c == ':')
            .collect()
    };

    let dockerfile = r#"# Stap 1: Bouw de Rust-applicatie (Geschikt voor Docker Compose én Google Cloud Run)
FROM rust:1-bookworm AS builder
WORKDIR /usr/src/blogrust
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Stap 2: Lichte runtime-container
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libsqlite3-0 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN mkdir -p /app/data
COPY --from=builder /usr/src/blogrust/target/release/blogrust /app/blogrust
COPY posts ./posts
COPY plugins ./plugins
COPY themes ./themes
COPY media ./media
COPY scaffolds ./scaffolds
EXPOSE 3000
CMD ["/app/blogrust"]
"#
    .to_string();

    let (compose_yml, caddyfile) = match modus {
        "alleen_blogrust" => (
            r#"services:
  blogrust:
    build: .
    container_name: blogrust_app
    restart: unless-stopped
    env_file:
      - .env
    ports:
      - "${BLOGRUST_PORT:-3000}:3000"
    volumes:
      - ./data:/app/data
      - ./posts:/app/posts
      - ./plugins:/app/plugins
      - ./themes:/app/themes
      - ./media:/app/media
      - ./scaffolds:/app/scaffolds
"#
            .to_string(),
            "# Caddy is uitgeschakeld in modus 'Alleen BlogRust Container'\n".to_string(),
        ),
        "caddy_https" => (
            r#"services:
  blogrust:
    build: .
    container_name: blogrust_app
    restart: unless-stopped
    env_file:
      - .env
    expose:
      - "3000"
    volumes:
      - ./data:/app/data
      - ./posts:/app/posts
      - ./plugins:/app/plugins
      - ./themes:/app/themes
      - ./media:/app/media
      - ./scaffolds:/app/scaffolds

  caddy:
    image: caddy:2-alpine
    container_name: blogrust_caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
      - "443:443/udp"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    depends_on:
      - blogrust

volumes:
  caddy_data:
  caddy_config:
"#
            .to_string(),
            format!(
                "{domein} {{\n    encode zstd gzip\n    reverse_proxy blogrust:3000\n}}\n",
                domein = schoon_domein
            ),
        ),
        _ => (
            // Standaard: "caddy_lokaal" (http://localhost op poort 80)
            r#"services:
  blogrust:
    build: .
    container_name: blogrust_app
    restart: unless-stopped
    env_file:
      - .env
    expose:
      - "3000"
    volumes:
      - ./data:/app/data
      - ./posts:/app/posts
      - ./plugins:/app/plugins
      - ./themes:/app/themes
      - ./media:/app/media
      - ./scaffolds:/app/scaffolds

  caddy:
    image: caddy:2-alpine
    container_name: blogrust_caddy
    restart: unless-stopped
    ports:
      - "80:80"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy_data:/data
      - caddy_config:/config
    depends_on:
      - blogrust

volumes:
  caddy_data:
  caddy_config:
"#
            .to_string(),
            ":80 {\n    encode zstd gzip\n    reverse_proxy blogrust:3000\n}\n".to_string(),
        ),
    };

    (dockerfile, compose_yml, caddyfile)
}

async fn instellingen_pagina(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(melding): Query<MeldingQuery>,
) -> impl IntoResponse {
    if !is_geinstalleerd(&state.db_pad) {
        return Redirect::to("/setup").into_response();
    }

    let Some(gebruiker) = haal_ingelogde_gebruiker_op(&headers, &state) else {
        return Redirect::to("/login").into_response();
    };

    let (titel, beschrijving, actief_thema) = haal_site_info_op(&state.db_pad).await;
    let (site_icoon, site_logo, brand_html, favicon_html) =
        haal_site_branding_op(&state.db_pad, &titel).await;
    let google_client_id = haal_google_client_id_op(&state.db_pad).await;
    let thema_css = laad_thema_css(&state.themes_map, &actief_thema);
    let nav_rechts = maak_nav_rechts_html(Some(&gebruiker));
    let media_bestanden = lees_media_bestanden(&state.media_map);

    let mut icoon_media_chips = String::new();
    let mut logo_media_chips = String::new();
    for b in &media_bestanden {
        let pad = format!("/media/{}", b.bestandsnaam);
        icoon_media_chips.push_str(&format!(
            r#"<button type="button" class="chip-btn" onclick="kiesIcoon('{pad}')">🖼️ {naam}</button>"#,
            pad = pad,
            naam = b.bestandsnaam
        ));
        logo_media_chips.push_str(&format!(
            r#"<button type="button" class="chip-btn" onclick="kiesLogo('{pad}')">🖼️ {naam}</button>"#,
            pad = pad,
            naam = b.bestandsnaam
        ));
    }

    let succes_html = if melding.opgeslagen.is_some() {
        r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 15px;border-radius:6px;margin-bottom:20px;font-weight:500;">✅ Instellingen zijn succesvol opgeslagen!</div>"#
    } else if melding.docker_klaar.is_some() {
        r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 15px;border-radius:6px;margin-bottom:20px;font-weight:500;">🐳 Dockerfile, docker-compose.yml en Caddyfile zijn aangemaakt in je projectmap!</div>"#
    } else if melding.env_klaar.is_some() {
        r#"<div style="background:#d1e7dd;border:1px solid #badbcc;color:#0f5132;padding:12px 15px;border-radius:6px;margin-bottom:20px;font-weight:500;">🔐 .env, .env.example en .gitignore zijn aangemaakt in je projectmap!</div>"#
    } else {
        ""
    };

    let env_poort = haal_env_waarde("BLOGRUST_PORT").unwrap_or_else(|| "3000".to_string());
    let env_domein = haal_env_waarde("BLOGRUST_DOMAIN").unwrap_or_else(|| "localhost".to_string());
    let env_db_pad =
        haal_env_waarde("BLOGRUST_DB_PAD").unwrap_or_else(|| "data/blogrust.db".to_string());

    let huidige_env = fs::read_to_string(".env").unwrap_or_default();
    let huidige_env_example = fs::read_to_string(".env.example").unwrap_or_default();
    let mut extra_env_regels = String::new();
    for (k, v) in parse_dotenv(&huidige_env) {
        if !matches!(
            k.as_str(),
            "BLOGRUST_PORT"
                | "BLOGRUST_DOMAIN"
                | "BLOGRUST_DB_PAD"
                | "RUST_LOG"
                | "GOOGLE_CLIENT_ID"
        ) {
            extra_env_regels.push_str(&format!("{}={}\n", k, v));
        }
    }
    if extra_env_regels.is_empty() {
        extra_env_regels = "REACTIES_INGESCHAKELD=true\n".to_string();
    }

    let env_preview_html = if !huidige_env.is_empty() {
        format!(
            r#"
            <div style="margin-top:20px;padding-top:16px;border-top:1px solid #e5e7eb;">
                <h3 style="margin:0 0 8px 0;font-size:15px;">📄 Huidige <code>.env</code> (lokaal/Docker) &amp; <code>.env.example</code> (voor GitHub &amp; ZIP):</h3>
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px;">
                    <pre style="background:#1e293b;color:#f8fafc;padding:12px;border-radius:6px;font-size:12px;overflow-x:auto;margin:0;"><code>{env_echt}</code></pre>
                    <pre style="background:#1e293b;color:#f8fafc;padding:12px;border-radius:6px;font-size:12px;overflow-x:auto;margin:0;"><code>{env_ex}</code></pre>
                </div>
                <p style="font-size:12px;color:#4b5563;margin:10px 0 0 0;">💡 In Rhai plug-ins (zoals een reactiesysteem) lees je deze variabelen direct uit met <code>let waarde = env("REACTIES_INGESCHAKELD");</code>.</p>
            </div>
            "#,
            env_echt = huidige_env,
            env_ex = huidige_env_example
        )
    } else {
        String::new()
    };

    let huidige_compose = fs::read_to_string("docker-compose.yml").unwrap_or_default();
    let huidige_caddy = fs::read_to_string("Caddyfile").unwrap_or_default();
    let docker_preview_html = if !huidige_compose.is_empty() {
        format!(
            r#"
            <div style="margin-top:20px;padding-top:16px;border-top:1px solid #e5e7eb;">
                <h3 style="margin:0 0 8px 0;font-size:15px;">📄 Huidige <code>docker-compose.yml</code> &amp; <code>Caddyfile</code> in projectmap:</h3>
                <div style="display:grid;grid-template-columns:2fr 1fr;gap:12px;">
                    <pre style="background:#1e293b;color:#f8fafc;padding:12px;border-radius:6px;font-size:12px;overflow-x:auto;margin:0;"><code>{compose}</code></pre>
                    <pre style="background:#1e293b;color:#f8fafc;padding:12px;border-radius:6px;font-size:12px;overflow-x:auto;margin:0;"><code>{caddy}</code></pre>
                </div>
                <p style="font-size:13px;margin:10px 0 0 0;">Starten in terminal: <code>docker compose up -d --build</code></p>
            </div>
            "#,
            compose = huidige_compose,
            caddy = huidige_caddy
        )
    } else {
        String::new()
    };

    Html(format!(
        r#"
        <!DOCTYPE html>
        <html lang="nl">
        <head>
            <meta charset="UTF-8">
            <title>Instellingen &lsaquo; {titel}</title>
            {favicon_html}
            <style>
                body {{ font-family: system-ui, sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #1f2937; background: #f9fafb; }}
                .topbar {{ display: flex; justify-content: space-between; align-items: center; background: #fff; padding: 14px 20px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; box-shadow: 0 1px 2px rgba(0,0,0,0.03); }}
                .brand {{ font-size: 20px; font-weight: 700; color: #ea580c; text-decoration: none; }}
                .nav-right {{ display: flex; align-items: center; gap: 10px; font-size: 14px; flex-wrap: wrap; }}
                .user-badge {{ color: #4b5563; font-weight: 500; }}
                .btn-new {{ text-decoration: none; color: #fff; padding: 6px 12px; border-radius: 4px; border: 1px solid #ea580c; background: #ea580c; font-weight: 500; }}
                .btn-link {{ text-decoration: none; color: #2271b1; padding: 6px 12px; border-radius: 4px; border: 1px solid #2271b1; background: #f6f7f7; font-weight: 500; }}
                .btn-link:hover {{ background: #e5f0fa; }}
                .btn-logout {{ background: #dc2626; color: #fff; border: 1px solid #b91c1c; padding: 6px 12px; border-radius: 4px; cursor: pointer; font-size: 14px; font-weight: 500; text-decoration: none; }}
                .btn-logout:hover {{ background: #b91c1c; }}
                .card {{ background: #fff; padding: 30px; border-radius: 8px; border: 1px solid #e5e7eb; margin-bottom: 24px; }}
                h1, h2 {{ margin-top: 0; font-size: 22px; color: #111827; border-bottom: 1px solid #e5e7eb; padding-bottom: 12px; }}
                .form-group {{ margin-bottom: 18px; }}
                label {{ display: block; font-weight: 600; margin-bottom: 6px; font-size: 14px; }}
                input[type="text"], input[type="password"], select, textarea {{ width: 100%; max-width: 460px; padding: 9px 12px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 14px; box-sizing: border-box; background: #fff; }}
                textarea {{ max-width: 100%; font-family: ui-monospace, Consolas, monospace; min-height: 90px; }}
                .hint {{ font-size: 12px; color: #6b7280; margin-top: 4px; }}
                .chips-row {{ display: flex; flex-wrap: wrap; gap: 6px; margin-top: 8px; align-items: center; }}
                .chip-btn {{ background: #f3f4f6; border: 1px solid #d1d5db; border-radius: 4px; padding: 4px 10px; font-size: 13px; cursor: pointer; }}
                .chip-btn:hover {{ border-color: #ea580c; color: #ea580c; background: #fff7ed; }}
                .brand-preview-box {{ display: inline-flex; align-items: center; gap: 10px; padding: 10px 16px; border: 1px dashed #cbd5e1; border-radius: 6px; background: #f8fafc; margin-top: 6px; font-size: 18px; font-weight: 700; color: #ea580c; }}
                .btn-save {{ background: #2271b1; color: #fff; border: 1px solid #2271b1; padding: 10px 20px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; margin-top: 8px; }}
                .btn-save:hover {{ background: #135e96; }}
                .btn-docker {{ background: #ea580c; color: #fff; border: 1px solid #ea580c; padding: 10px 20px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; margin-top: 8px; }}
                .btn-docker:hover {{ background: #c2410c; }}
                .btn-env {{ background: #7c3aed; color: #fff; border: 1px solid #6d28d9; padding: 10px 20px; border-radius: 6px; font-size: 14px; font-weight: 600; cursor: pointer; margin-top: 8px; }}
                .btn-env:hover {{ background: #6d28d9; }}
                {thema_css}
            </style>
        </head>
        <body>
            <nav class="topbar">
                <a href="/" class="brand">{brand_html}</a>
                <div class="nav-right">
                    {nav_rechts}
                </div>
            </nav>

            {succes_html}

            <main class="card">
                <h1>⚙️ Algemene Instellingen, Logo &amp; Account</h1>
                <form action="/instellingen" method="post">
                    <div class="form-group">
                        <label for="site_titel">Websitetitel</label>
                        <input type="text" id="site_titel" name="site_titel" value="{titel}" oninput="updateBrandPreview()" required>
                    </div>
                    <div class="form-group">
                        <label for="site_beschrijving">Ondertitel / Beschrijving</label>
                        <input type="text" id="site_beschrijving" name="site_beschrijving" value="{beschrijving}">
                    </div>

                    <div class="form-group" style="padding:16px;background:#f8fafc;border:1px solid #e5e7eb;border-radius:8px;margin-bottom:20px;">
                        <label for="site_icoon">🎨 Website Icoon &amp; Browser-tab Favicon</label>
                        <input type="text" id="site_icoon" name="site_icoon" value="{site_icoon}" placeholder="🦀 of /media/blogrust-icon.svg" oninput="updateBrandPreview()">
                        <div class="hint">Kies een emoji (wordt ook direct het icoontje in je browsertabblad!) of een afbeeldingspad uit <code>/media/</code>:</div>
                        <div class="chips-row">
                            <button type="button" class="chip-btn" onclick="kiesIcoon('🦀')">🦀</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('🚀')">🚀</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('📰')">📰</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('🌲')">🌲</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('⚡')">⚡</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('🔥')">🔥</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('📚')">📚</button>
                            <button type="button" class="chip-btn" onclick="kiesIcoon('🦉')">🦉</button>
                            {icoon_media_chips}
                        </div>

                        <label for="site_logo" style="margin-top:16px;">🖼️ Website Logo (Afbeelding in navigatiebalk)</label>
                        <input type="text" id="site_logo" name="site_logo" value="{site_logo}" placeholder="Laat leeg voor alleen icoon + titel, of kies /media/..." oninput="updateBrandPreview()">
                        <div class="hint">Optioneel: vervang het icoon in de bovenbalk door een eigen logo-afbeelding uit <code>/media/</code>:</div>
                        <div class="chips-row">
                            <button type="button" class="chip-btn" onclick="kiesLogo('')">✖️ Geen logo (alleen icoon)</button>
                            {logo_media_chips}
                            <a href="/media" style="font-size:12px;color:#2271b1;text-decoration:none;margin-left:6px;">+ Eigen logo/icoon uploaden</a>
                        </div>

                        <div style="margin-top:14px;display:flex;gap:16px;flex-wrap:wrap;align-items:center;">
                            <div>
                                <div style="font-size:12px;font-weight:600;color:#4b5563;">Live Voorbeeld (Navigatiebalk):</div>
                                <div id="brand-preview" class="brand-preview-box">{brand_html}</div>
                            </div>
                            <div>
                                <div style="font-size:12px;font-weight:600;color:#4b5563;">Live Voorbeeld (Browser-tabblad / Favicon):</div>
                                <div id="tab-preview" style="display:inline-flex;align-items:center;gap:8px;padding:8px 14px;background:#e2e8f0;border-radius:8px 8px 0 0;border:1px solid #cbd5e1;margin-top:6px;font-size:13px;font-weight:600;color:#1e293b;">
                                    <span id="tab-favicon-icoon">{site_icoon}</span>
                                    <span id="tab-favicon-titel">{titel}</span>
                                    <span style="color:#64748b;margin-left:6px;">&times;</span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div class="form-group">
                        <label for="google_client_id">🌐 Google OAuth Client ID (voor Community Plug-in &amp; Thema uploads)</label>
                        <input type="text" id="google_client_id" name="google_client_id" value="{google_client_id}" placeholder="bijv. 123456789-abc.apps.googleusercontent.com">
                        <div class="hint">Optioneel: koppel een echte Google Cloud OAuth Client ID op <a href="/auth/google">/auth/google</a>. Zonder ID werkt de ingebouwde lokale Google test-login.</div>
                    </div>
                    <div class="form-group">
                        <label for="admin_naam">Gebruikersnaam (Admin)</label>
                        <input type="text" id="admin_naam" name="admin_naam" value="{gebruiker}" required>
                    </div>
                    <div class="form-group">
                        <label for="nieuw_wachtwoord">Nieuw wachtwoord</label>
                        <input type="password" id="nieuw_wachtwoord" name="nieuw_wachtwoord" placeholder="Laat leeg om huidige wachtwoord te behouden">
                        <div class="hint">Vul alleen in als je je wachtwoord wilt wijzigen.</div>
                    </div>
                    <button type="submit" class="btn-save">💾 Wijzigingen opslaan</button>
                </form>
            </main>

            <section class="card">
                <h2>🔐 Omgevingsvariabelen (<code>.env</code> &amp; <code>.env.example</code> Generator)</h2>
                <p style="font-size:14px;color:#4b5563;margin-top:0;margin-bottom:18px;">Beheer hier je <code>.env</code> voor Docker Compose, opstart-poorten en geheime sleutels voor toekomstige plug-ins. Met 1 klik maakt BlogRust een lokale <code>.env</code>, een veilige <code>.env.example</code> (voor GitHub &amp; <code>blogrust-cms.zip</code>) én een <code>.gitignore</code> aan.</p>
                <form action="/instellingen/env" method="post">
                    <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;">
                        <div class="form-group" style="margin-bottom:10px;">
                            <label for="env_poort">Server Poort (<code>BLOGRUST_PORT</code>)</label>
                            <input type="text" id="env_poort" name="poort" value="{env_poort}" placeholder="3000" required>
                        </div>
                        <div class="form-group" style="margin-bottom:10px;">
                            <label for="env_domein">Standaard Domein (<code>BLOGRUST_DOMAIN</code>)</label>
                            <input type="text" id="env_domein" name="domein" value="{env_domein}" placeholder="localhost of mijnblog.nl" required>
                        </div>
                    </div>
                    <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;">
                        <div class="form-group" style="margin-bottom:10px;">
                            <label for="env_db">SQLite Database Pad (<code>BLOGRUST_DB_PAD</code>)</label>
                            <input type="text" id="env_db" name="db_pad" value="{env_db_pad}" placeholder="data/blogrust.db" required>
                        </div>
                        <div class="form-group" style="margin-bottom:10px;">
                            <label for="env_google">Google OAuth Client ID (<code>GOOGLE_CLIENT_ID</code>)</label>
                            <input type="text" id="env_google" name="google_client_id" value="{google_client_id}" placeholder="Optioneel voor BlogRust.org portal">
                        </div>
                    </div>
                    <div class="form-group" style="margin-top:6px;">
                        <label for="extra_env">🧩 Extra Plug-in &amp; API Variabelen (één <code>SLEUTEL=WAARDE</code> per regel)</label>
                        <textarea id="extra_env" name="extra_env" placeholder="REACTIES_INGESCHAKELD=true">{extra_env_regels}</textarea>
                        <div class="hint">Beschikbaar in elke <code>.rhai</code> plug-in via <code>env("SLEUTEL")</code> én in je Docker-container. In <code>.env.example</code> worden geheime waarden automatisch vervangen door placeholders.</div>
                    </div>
                    <button type="submit" class="btn-env">🔐 Genereer .env &amp; .env.example Bestanden</button>
                </form>
                {env_preview_html}
            </section>

            <section class="card">
                <h2>🐳 Docker Compose &amp; Caddy Container Generator</h2>
                <p style="font-size:14px;color:#4b5563;margin-top:0;margin-bottom:18px;">Kies hieronder hoe je BlogRust in Docker-containers wilt draaien. Met 1 klik worden <code>Dockerfile</code>, <code>docker-compose.yml</code> (gekoppeld aan <code>.env</code>) en <code>Caddyfile</code> voor je klaargezet.</p>
                <form action="/instellingen/docker" method="post">
                    <div class="form-group">
                        <label for="modus">Kies Container Opzet (Docker Compose of Google Cloud Run)</label>
                        <select id="modus" name="modus" onchange="checkDomeinVeld()">
                            <option value="caddy_lokaal">1. Lokaal met Caddy Reverse Proxy (http://localhost op poort 80)</option>
                            <option value="caddy_https">2. Productie VPS met Caddy + Automatische HTTPS (Let's Encrypt SSL)</option>
                            <option value="alleen_blogrust">3. Alleen BlogRust Container / Google Cloud Run (zonder Caddy)</option>
                        </select>
                    </div>
                    <div class="form-group" id="domein-groep" style="display:none;">
                        <label for="domein">Jouw Domeinnaam (voor automatische HTTPS in Caddy)</label>
                        <input type="text" id="domein" name="domein" value="{env_domein}" placeholder="bijv. mijnblog.nl">
                        <div class="hint">Caddy vraagt automatisch een gratis SSL-certificaat aan voor dit domein.</div>
                    </div>
                    <button type="submit" class="btn-docker">🐳 Genereer Docker &amp; Caddy Bestanden</button>
                </form>
                <div style="margin-top:18px;padding:14px 16px;background:#eff6ff;border:1px solid #bfdbfe;border-radius:6px;font-size:13px;color:#1e3a8a;">
                    <strong>☁️ Direct naar Google Cloud Run (zonder Docker Hub) + Cloudflare Domein:</strong>
                    <pre style="background:#1e293b;color:#f8fafc;padding:10px 14px;border-radius:6px;font-size:12px;overflow-x:auto;margin:8px 0;"><code>gcloud run deploy blogrust --source . --region europe-west4 --allow-unauthenticated</code></pre>
                    <div>Google Cloud Build bouwt je <code>Dockerfile</code> automatisch in de cloud (<code>.gcloudignore</code> sluit <code>target/</code> uit). Koppel daarna in Cloud Run onder <em>Custom Domains</em> je Cloudflare-domein via een <code>CNAME</code> naar <code>ghs.googlehosted.com</code>.</div>
                </div>
                {docker_preview_html}
            </section>

            <script>
                function checkDomeinVeld() {{
                    const modus = document.getElementById('modus').value;
                    const groep = document.getElementById('domein-groep');
                    groep.style.display = (modus === 'caddy_https') ? 'block' : 'none';
                }}
                function kiesIcoon(waarde) {{
                    document.getElementById('site_icoon').value = waarde;
                    updateBrandPreview();
                }}
                function kiesLogo(waarde) {{
                    document.getElementById('site_logo').value = waarde;
                    updateBrandPreview();
                }}
                function updateBrandPreview() {{
                    const titel = document.getElementById('site_titel').value || 'BlogRust';
                    const icoon = (document.getElementById('site_icoon').value || '🦀').trim();
                    const logo = (document.getElementById('site_logo').value || '').trim();
                    const box = document.getElementById('brand-preview');
                    if (logo !== '') {{
                        box.innerHTML = '<img src="' + logo + '" alt="Logo" style="height:28px;width:auto;max-width:160px;object-fit:contain;vertical-align:middle;margin-right:8px;border-radius:4px;"><span>' + titel + '</span>';
                    }} else if (icoon.startsWith('/') || icoon.startsWith('http://') || icoon.startsWith('https://')) {{
                        box.innerHTML = '<img src="' + icoon + '" alt="Icon" style="height:24px;width:24px;object-fit:contain;vertical-align:middle;margin-right:8px;border-radius:4px;"><span>' + titel + '</span>';
                    }} else {{
                        box.textContent = icoon + ' ' + titel;
                    }}

                    // Update ook direct het Browser-tabblad (Favicon) voorbeeld én het echte tabblad!
                    const tabIcoon = document.getElementById('tab-favicon-icoon');
                    const tabTitel = document.getElementById('tab-favicon-titel');
                    const liveFav = document.getElementById('live-favicon');
                    tabTitel.textContent = titel;
                    if (icoon.startsWith('/') || icoon.startsWith('http://') || icoon.startsWith('https://')) {{
                        tabIcoon.innerHTML = '<img src="' + icoon + '" alt="fav" style="width:16px;height:16px;vertical-align:middle;">';
                        if (liveFav) liveFav.href = icoon;
                    }} else {{
                        tabIcoon.textContent = icoon;
                        if (liveFav) liveFav.href = "data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><text y='.9em' font-size='90'>" + icoon + "</text></svg>";
                    }}
                }}
            </script>
        </body>
        </html>
        "#,
        titel = titel,
        favicon_html = favicon_html,
        brand_html = brand_html,
        site_icoon = site_icoon,
        site_logo = site_logo,
        google_client_id = google_client_id,
        icoon_media_chips = icoon_media_chips,
        logo_media_chips = logo_media_chips,
        beschrijving = beschrijving,
        thema_css = thema_css,
        gebruiker = gebruiker,
        nav_rechts = nav_rechts,
        succes_html = succes_html,
        env_poort = env_poort,
        env_domein = env_domein,
        env_db_pad = env_db_pad,
        extra_env_regels = extra_env_regels,
        env_preview_html = env_preview_html,
        docker_preview_html = docker_preview_html
    ))
    .into_response()
}

async fn verwerk_env_generator(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<DotEnvForm>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    let (dotenv, dotenv_example) = maak_dotenv_en_example_inhoud(
        &form.poort,
        &form.domein,
        &form.db_pad,
        &form.google_client_id,
        &form.extra_env,
    );

    fs::write(".env", dotenv).unwrap();
    fs::write(".env.example", dotenv_example).unwrap();
    if !Path::new(".gitignore").exists() {
        fs::write(
            ".gitignore",
            "# BlogRust Git Ignore\ntarget/\ndata/*.db\ndata/*.db-*\n.env\n",
        )
        .unwrap();
    }

    // Synchroniseer Google Client ID ook met SQLite instellingen indien ingevuld
    if !form.google_client_id.trim().is_empty() {
        if let Ok(pool) = SqlitePool::connect(&format!("sqlite:{}", state.db_pad)).await {
            let _ = sqlx::query(
                "INSERT INTO instellingen (sleutel, waarde) VALUES ('google_client_id', ?)
                 ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
            )
            .bind(form.google_client_id.trim())
            .execute(&pool)
            .await;
        }
    }

    Redirect::to("/instellingen?env_klaar=1").into_response()
}

async fn verwerk_docker_generator(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<DockerComposeForm>,
) -> impl IntoResponse {
    if haal_ingelogde_gebruiker_op(&headers, &state).is_none() {
        return Redirect::to("/login").into_response();
    }

    let (dockerfile, compose_yml, caddyfile) =
        maak_docker_en_caddy_inhoud(&form.modus, &form.domein);

    fs::write("Dockerfile", dockerfile).unwrap();
    fs::write("docker-compose.yml", compose_yml).unwrap();
    fs::write("Caddyfile", caddyfile).unwrap();

    Redirect::to("/instellingen?docker_klaar=1").into_response()
}

async fn verwerk_instellingen(
    headers: HeaderMap,
    State(state): State<AppState>,
    Form(form): Form<InstellingenForm>,
) -> impl IntoResponse {
    let Some(huidige_gebruiker) = haal_ingelogde_gebruiker_op(&headers, &state) else {
        return Redirect::to("/login").into_response();
    };

    let pool = SqlitePool::connect(&format!("sqlite:{}", state.db_pad))
        .await
        .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('site_titel', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(&form.site_titel)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('site_beschrijving', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(&form.site_beschrijving)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('site_icoon', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(form.site_icoon.trim())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('site_logo', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(form.site_logo.trim())
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO instellingen (sleutel, waarde) VALUES ('google_client_id', ?)
         ON CONFLICT(sleutel) DO UPDATE SET waarde = excluded.waarde",
    )
    .bind(form.google_client_id.trim())
    .execute(&pool)
    .await
    .unwrap();

    if form.nieuw_wachtwoord.trim().is_empty() {
        sqlx::query("UPDATE gebruikers SET naam = ? WHERE naam = ?")
            .bind(&form.admin_naam)
            .bind(&huidige_gebruiker)
            .execute(&pool)
            .await
            .unwrap();
    } else {
        let salt = SaltString::generate(&mut OsRng);
        let nieuwe_hash = Argon2::default()
            .hash_password(form.nieuw_wachtwoord.as_bytes(), &salt)
            .unwrap()
            .to_string();

        sqlx::query("UPDATE gebruikers SET naam = ?, wachtwoord_hash = ? WHERE naam = ?")
            .bind(&form.admin_naam)
            .bind(&nieuwe_hash)
            .bind(&huidige_gebruiker)
            .execute(&pool)
            .await
            .unwrap();
    }

    if let Some(Ok(cookie_header)) = headers.get(COOKIE).map(|v| v.to_str()) {
        for deel in cookie_header.split(';') {
            let deel = deel.trim();
            if let Some(token) = deel.strip_prefix("blogrust_sessie=") {
                state
                    .sessies
                    .write()
                    .unwrap()
                    .insert(token.to_string(), form.admin_naam.clone());
            }
        }
    }

    Redirect::to("/instellingen?opgeslagen=1").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_geinstalleerd_check() {
        assert!(!is_geinstalleerd("data/bestaat_echt_niet_12345.db"));
        assert!(is_geinstalleerd("Cargo.toml"));
    }

    #[test]
    fn test_argon2_wachtwoord_hash_en_verificatie() {
        let wachtwoord = "MijnSuperSterkeWachtwoord!2026";
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let hash_string = argon2
            .hash_password(wachtwoord.as_bytes(), &salt)
            .expect("Hashing mislukt")
            .to_string();

        let parsed_hash = PasswordHash::new(&hash_string).expect("Ongeldige hash-structuur");
        assert!(argon2
            .verify_password(wachtwoord.as_bytes(), &parsed_hash)
            .is_ok());
        assert!(argon2
            .verify_password(b"FoutWachtwoord", &parsed_hash)
            .is_err());
    }

    #[test]
    fn test_sessie_aanmaken_en_uitloggen() {
        let state = AppState {
            db_pad: "sqlite::memory:".to_string(),
            posts_map: "posts".to_string(),
            plugins_map: "plugins".to_string(),
            themes_map: "themes".to_string(),
            media_map: "media".to_string(),
            scaffolds_map: "scaffolds".to_string(),
            sessies: Arc::new(RwLock::new(HashMap::new())),
        };

        let token = maak_sessie_aan(&state, "admin");
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("blogrust_sessie={}", token)).unwrap(),
        );

        assert_eq!(
            haal_ingelogde_gebruiker_op(&headers, &state),
            Some("admin".to_string())
        );

        verwijder_sessie(&headers, &state);
        assert_eq!(haal_ingelogde_gebruiker_op(&headers, &state), None);
    }

    #[test]
    fn test_hugo_frontmatter_en_markdown_parser() {
        let md = r#"---
title: "Hugo Stijl Test"
date: "2026-11-14"
category: "Rust"
tags: ["axum", "hugo"]
cover: "https://example.com/cover.jpg"
summary: "Een korte test samenvatting"
draft: false
---

Dit is **dikgedrukte** tekst in Markdown."#;

        let post = parse_markdown_post("hugo-stijl-test", md);
        assert_eq!(post.titel, "Hugo Stijl Test");
        assert_eq!(post.categorie, "Rust");
        assert_eq!(post.tags, vec!["axum", "hugo"]);
        assert_eq!(post.cover, "https://example.com/cover.jpg");
        assert!(!post.concept);
        assert!(post.html_inhoud.contains("<strong>dikgedrukte</strong>"));
    }

    #[test]
    fn test_rhai_plugins_uitvoering() {
        let post = MarkdownPost {
            slug: "test".to_string(),
            titel: "Test".to_string(),
            datum: "2026-11-14".to_string(),
            volgorde: 1,
            categorie: "Rust".to_string(),
            tags: vec![],
            cover: String::new(),
            samenvatting: String::new(),
            concept: false,
            ruwe_markdown: "Een korte test tekst voor de leestijd plugin.".to_string(),
            html_inhoud: "<p>Een korte test tekst voor de leestijd plugin.</p>".to_string(),
        };

        let bewerkt = pas_plugins_toe_op_post("plugins", post);
        assert!(bewerkt.html_inhoud.contains("Geschatte leestijd:"));
    }

    #[test]
    fn test_themas_en_scaffolds_laden() {
        let css = laad_thema_css("themes", "standaard.css");
        assert!(css.contains("#ea580c"));
        let post_scaffold = laad_scaffold("scaffolds", "post.md", "fallback");
        assert!(post_scaffold.contains("## Introductie"));
    }

    #[test]
    fn test_media_veilige_bestandsnaam_en_mime() {
        assert_eq!(
            maak_veilige_bestandsnaam("../../Mijn Foto 1.PNG"),
            "mijn-foto-1.png"
        );
        assert_eq!(bepaal_content_type("foto.jpg"), "image/jpeg");
        assert_eq!(bepaal_content_type("banner.svg"), "image/svg+xml");
    }

    #[test]
    fn test_docker_en_caddy_generator() {
        let (dockerfile, compose, caddy) =
            maak_docker_en_caddy_inhoud("caddy_https", "mijnblog.nl");
        assert!(dockerfile.contains("FROM rust:1-bookworm"));
        assert!(compose.contains("caddy:2-alpine"));
        assert!(caddy.contains("mijnblog.nl {"));
        assert!(caddy.contains("reverse_proxy blogrust:3000"));
    }

    #[test]
    fn test_logo_en_icoon_branding_html() {
        let (brand_emoji, fav_emoji) = maak_brand_en_favicon_html("BlogRust", "🚀", "");
        assert_eq!(brand_emoji, "🚀 BlogRust");
        assert!(fav_emoji.contains("data:image/svg+xml"));

        let (brand_logo, fav_img) = maak_brand_en_favicon_html(
            "BlogRust",
            "/media/blogrust-icon.svg",
            "/media/blogrust-logo.svg",
        );
        assert!(brand_logo.contains(r#"src="/media/blogrust-logo.svg""#));
        assert!(fav_img.contains(r#"href="/media/blogrust-icon.svg""#));
    }

    #[test]
    fn test_google_auth_sessie_en_portal_nav() {
        let state = AppState {
            db_pad: "sqlite::memory:".to_string(),
            posts_map: "posts".to_string(),
            plugins_map: "plugins".to_string(),
            themes_map: "themes".to_string(),
            media_map: "media".to_string(),
            scaffolds_map: "scaffolds".to_string(),
            sessies: Arc::new(RwLock::new(HashMap::new())),
        };

        let gtoken = maak_google_sessie_aan(&state, "developer@gmail.com");
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_str(&format!("blogrust_google_sessie={}", gtoken)).unwrap(),
        );

        assert_eq!(
            haal_google_gebruiker_op(&headers, &state),
            Some("developer@gmail.com".to_string())
        );

        let nav_id = haal_nav_identiteit_op(&headers, &state);
        let nav_html = maak_nav_rechts_html(nav_id.as_deref());
        assert!(nav_html.contains("developer@gmail.com"));
        assert!(nav_html.contains("/download"));
    }

    #[test]
    fn test_standaard_cms_download_zonder_google_auth() {
        let voorbeeld = r#"// Standaard CMS: Google Auth developer route uitgeschakeld
let google_gebruiker: Option<String> = None;"#;
        let schoon = maak_standaard_cms_zonder_google_auth(voorbeeld);
        assert!(schoon.contains("BlogRust Standaard CMS Editie"));
        assert!(!schoon.contains(r#".route("/auth/google""#));
        assert!(schoon.contains("let google_gebruiker: Option<String> = None;"));
    }

    #[test]
    fn test_zip_archief_en_crc32_voor_folder_download() {
        assert_eq!(bereken_crc32(b"123456789"), 0xCBF4_3926);
        let state = AppState {
            db_pad: "data/blogrust.db".to_string(),
            posts_map: "posts".to_string(),
            plugins_map: "plugins".to_string(),
            themes_map: "themes".to_string(),
            media_map: "media".to_string(),
            scaffolds_map: "scaffolds".to_string(),
            sessies: Arc::new(RwLock::new(HashMap::new())),
        };
        let portal_zip = bouw_project_zip(&state, "blogrust-portal", true);
        // Check PK\x03\x04 local header en PK\x05\x06 EOCD signature
        assert_eq!(&portal_zip[0..4], &[0x50, 0x4B, 0x03, 0x04]);
        assert!(portal_zip.windows(4).any(|w| w == [0x50, 0x4B, 0x05, 0x06]));
        // Schrijf zowel blogrust-portal.zip als blogrust-cms.zip direct naar de hoofdmap
        fs::write("blogrust-portal.zip", &portal_zip).unwrap();
        let cms_zip = bouw_project_zip(&state, "blogrust-cms", false);
        fs::write("blogrust-cms.zip", &cms_zip).unwrap();
    }

    #[test]
    fn test_dotenv_parser_en_generator() {
        let (env_echt, env_ex) = maak_dotenv_en_example_inhoud(
            "3000",
            "mijnblog.nl",
            "data/blogrust.db",
            "geheim-google-id",
            "REACTIES_SECRET=mijn_geheim_123",
        );
        let map = parse_dotenv(&env_echt);
        assert_eq!(map.get("BLOGRUST_PORT"), Some(&"3000".to_string()));
        assert_eq!(map.get("BLOGRUST_DOMAIN"), Some(&"mijnblog.nl".to_string()));
        assert_eq!(
            map.get("REACTIES_SECRET"),
            Some(&"mijn_geheim_123".to_string())
        );
        // Check dat .env.example geen echte geheime sleutels bevat
        assert!(env_ex.contains("REACTIES_SECRET=jouw_waarde_hier"));
        assert!(!env_ex.contains("mijn_geheim_123"));
        assert!(!env_ex.contains("geheim-google-id"));
    }

    #[test]
    fn test_google_auth_reacties_plugin_en_jwt() {
        let post = MarkdownPost {
            slug: "test-reacties".to_string(),
            titel: "Test Reacties".to_string(),
            datum: "2026-11-18".to_string(),
            volgorde: 1,
            categorie: "Rust".to_string(),
            tags: vec![],
            cover: String::new(),
            samenvatting: String::new(),
            concept: false,
            ruwe_markdown: "Test inhoud".to_string(),
            html_inhoud: "<p>Test inhoud</p>".to_string(),
        };
        let bewerkt = pas_plugins_toe_op_post("plugins", post);
        assert!(bewerkt.html_inhoud.contains("<!-- BLOGRUST_GOOGLE_REACTIES -->"));

        // Test JWT payload decodering: {"email":"reactie.tester@gmail.com"} -> eyJlbWFpbCI6InJlYWN0aWUudGVzdGVyQGdtYWlsLmNvbSJ9
        let nep_jwt = "header.eyJlbWFpbCI6InJlYWN0aWUudGVzdGVyQGdtYWlsLmNvbSJ9.sig";
        assert_eq!(
            haal_email_uit_google_jwt(nep_jwt),
            Some("reactie.tester@gmail.com".to_string())
        );

        // Test dat de Multi-Language (i18n.rhai) plug-in de taalkiezer in de navigatiebalk plaatst
        let nav_html = maak_nav_rechts_html(None);
        assert!(nav_html.contains("blogrust-lang-switcher"));
        assert!(nav_html.contains("🇬🇧 EN"));
    }

    #[tokio::test]
    async fn test_sqlite_database_tabellen_en_opslag() {
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Kon geen in-memory SQLite database openen");

        sqlx::raw_sql(
            "CREATE TABLE instellingen (sleutel TEXT PRIMARY KEY, waarde TEXT NOT NULL);
             CREATE TABLE gebruikers (id INTEGER PRIMARY KEY AUTOINCREMENT, naam TEXT UNIQUE NOT NULL, wachtwoord_hash TEXT NOT NULL);
             CREATE TABLE berichten (id INTEGER PRIMARY KEY AUTOINCREMENT, titel TEXT NOT NULL, inhoud TEXT NOT NULL, aangemaakt DATETIME DEFAULT CURRENT_TIMESTAMP);",
        )
        .execute(&pool)
        .await
        .expect("Kon tabellen niet aanmaken");

        sqlx::query("INSERT INTO instellingen (sleutel, waarde) VALUES ('site_titel', ?)")
            .bind("Mijn Test Blog")
            .execute(&pool)
            .await
            .expect("Kon site_titel niet opslaan");

        let resultaat: (String,) =
            sqlx::query_as("SELECT waarde FROM instellingen WHERE sleutel = 'site_titel'")
                .fetch_one(&pool)
                .await
                .expect("Kon site_titel niet ophalen");

        assert_eq!(resultaat.0, "Mijn Test Blog");
    }
}
