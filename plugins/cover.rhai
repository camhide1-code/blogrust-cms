// Cover Afbeelding Plug-in voor BlogRust
fn bewerk_bericht_met_cover(tekst, cover, categorie) {
    if cover != "" {
        return "<img src=\"" + cover + "\" class=\"plugin-cover\" alt=\"Cover afbeelding\">\n" + tekst;
    }
    return tekst;
}
