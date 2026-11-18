// 💬 Google Auth Reactiesysteem Plug-in voor BlogRust
fn bewerk_bericht(tekst) {
    if env("REACTIES_INGESCHAKELD") != "false" {
        return tekst + "\n<!-- BLOGRUST_GOOGLE_REACTIES -->";
    }
    return tekst;
}
