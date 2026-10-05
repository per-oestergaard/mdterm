# Plan: Slidebilleder med tekstflow

Tilføj PNG/JPEG-billeder, der flyder til venstre eller højre for tekst på slides, med samme layout i TUI og SVG/PNG-slideeksport. Implementeringen er gennemført med de afgrænsninger, der står nedenfor.

## Valgt syntaks

```markdown
<!-- mdterm:wrap width=32% side=right -->
![Taleren på scenen](images/taler.jpg)
```

Kommentaren knyttes til det næste Markdown-billede. Standard Markdown-renderere ignorerer kommentaren og viser billedet normalt inline. Mdterm bruger det som float, men skjuler billedets almindelige flowlinjer. Alt-teksten forbliver en beskrivelse af billedet. Filnavnsmønstre fravælges.

## Scope

- Interaktiv slidevisning samt SVG- og PNG-slideeksport.
- PNG/JPEG som input, bredde angivet i procent og placering med `side=left|right`.
- Tekst flyder i den frie del af linjerne ved siden af billedets rektangel og bruger fuld bredde efter billedets bund.
- Ingen automatisk billedanalyse eller vilkårlige x/y-koordinater.
- Første version omfatter ikke ASCII-art som float, SVG som input eller HTML-eksport. Flere floats på samme slide får inline-fallback.
- Relative billedstier skal virke ens i TUI og eksport: brug kildedokumentets mappe, eller arbejdsmappe for stdin, sammen med eksisterende sikkerhedsvalidering.

## Implementeringstrin

1. [x] Udvid Markdown-parseren og dokumentmodellen til at opsnappe og validere `mdterm:wrap`-kommentarer, knytte instruktionen til næste Markdown-billede og håndtere ugyldige parametre med fallback. Almindelige billeder er uændrede.
2. [x] Beregn billedbredde ud fra procent og terminal-/eksportbredde, flow-wrap tekst omkring billedet, og afslut float ved billedets bund eller slidebrud.
3. [x] Integrér TUI-layoutet med `ImageCache`. Float-billeder tegnes med 2×2 Unicode-kvadrantblokke i præcis kolonneposition; billeder, der ikke kan floats, beholder den almindelige inline-visning.
4. [x] Tegn float-billede og tekst i SVG/PNG-eksport med indlejret rasterdata og kildebaserede lokale stier. ODP bruger samme slidegrafik.
5. [x] Tilføj fokuserede tests for parsing, ugyldige direktiver, stiopløsning, float-geometri, tekstflow, slidegrænser og SVG/PNG. Kør formatering, `cargo test` og `cargo clippy`.

## Eksisterende kodeområder

- `src/markdown.rs` — `Renderer::process`, billed- og HTML-eventhåndtering; horisontale regler bliver `LineMeta::SlideBreak`, og billeder bliver i dag flow-pladsholderlinjer.
- `src/style.rs` — linje-/dokumentmetadata og eventuelle fælles layouttyper.
- `src/viewer.rs` — `finalize_layout`, slidegrænser, TUI-rendering og brug af billedcache.
- `src/image.rs` — rasterindlæsning, cache og terminalprotokoller. Overlay af rasterbilleder og tekst afhænger af terminalunderstøttelsen.
- `src/export.rs` — eksisterende slideopdeling samt SVG/PNG-rendering. HTML-eksport opdeler ikke dokumentet i slides.
- `src/main.rs` — videregivelse af kildesti/base mappe til renderer og eksport.

## Afklaringer til implementeringen

- Side og bredde er valgt; gyldige bredder er 1–80%.
- HTML- og ODP-eksport samt ASCII- og SVG-input kan tilføjes senere uden at ændre kommentarkonventionen.

## Implementeringsafgrænsninger

- TUI-floats bruger 2×2 kvadrantblokke uanset den valgte protokol; umarkerede billeder bruger stadig den registrerede protokol.
- Float-input er lokale PNG/JPEG-filer. Fjernbilleder og andre formater falder tilbage til almindelig inline-visning.
- Kun én float pr. slide layoutes. Efterfølgende float-billeder på samme slide falder tilbage til inline-visning.
- TUI og eksport deler parsedokumentets floatmetadata og de samme bredde-/sidevalg, men beregner terminalrækker og SVG-pixels hver for sig ud fra deres respektive måleenheder.
