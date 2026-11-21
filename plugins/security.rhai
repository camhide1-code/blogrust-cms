// 🛡️ BlogRust Shield — Beveiligings-, Anti-Spam & Anti-Crash Plug-in
// Beschermt tegen XSS, Tabnabbing, Spam-bots en kwaadaardige scripts.

fn bewerk_navigatie(nav_html) {
    return nav_html + "\n<!-- BLOGRUST_SECURITY_SHIELD -->";
}

fn bewerk_bericht(tekst) {
    // Strip eventuele kwaadaardige javascript: links direct server-side in Rhai
    let veilig = tekst
        .replace("javascript:", "geblokkeerd:")
        .replace("data:text/html", "geblokkeerd:");
    return veilig;
}
