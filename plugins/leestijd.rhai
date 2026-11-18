// Dit script is geschreven door een externe plug-in bouwer
fn bewerk_bericht(tekst) {
    // Tel het aantal woorden door op spaties te splitsen
    let woorden = tekst.split(" ").len();
    
    // Gemiddelde leessnelheid is ongeveer 200 woorden per minuut
    let minuten = (woorden / 200) + 1;
    
    // Plak de berekende leestijd automatisch boven de tekst
    return "<p><em>⏱️ Geschatte leestijd: " + minuten + " minuut</em></p>\n" + tekst;
}
 