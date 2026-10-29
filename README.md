# nezty

Windows-desktopapp (Windows 10 en 11) die games soepeler laat draaien met eerlijke, omkeerbare tweaks. Gebouwd met Tauri 2, React, TypeScript en Tailwind, met een Rust-backend die het Windows-register, PowerShell en Win32-API's aanspreekt. De app is beschikbaar in het Nederlands, Engels en Frans.

## Wat de app doet

- **Dashboard**: live processor, videokaart, geheugen, temperatuur (als er een sensor is) en ping. De knop **Game Boost** past het gekozen profiel in één keer toe. Daarna toont een resultaatscherm wat er echt gebeurd is: hoeveel apps dicht zijn, hoeveel geheugen dat oplevert en welke tweaks direct werken of pas na een herstart van de game. Draait de game al, dan meet nezty op verzoek de FPS vóór en na (Intel PresentMon).
- **Tweaks, tabblad Per game**: vindt games in Epic, Steam, Battle.net, Riot, EA, de Xbox-app en Minecraft, met de echte afbeelding van de game. Afbeeldingen komen eerst lokaal uit de launcher (Steam-bibliotheekcache, Xbox/Store-bestanden, de Epic-catalogus), daarna van de Steam-CDN en optioneel van SteamGridDB (eigen sleutel in Instellingen). Per game een profiel, automatisch boosten bij de start en terugzetten bij het sluiten. Fortnite krijgt een eigen pagina: controle op de snelle videokaart, aanbevolen instellingen met één klik in `GameUserSettings.ini` (V-Sync, FPS-limiet, NVIDIA Reflex, Performance-modus, altijd met back-up en per sleutel terug te zetten), bouw- en edit-instellingen, overlays en hardwareversnelling van Discord en browsers. Minecraft en Counter-Strike 2 krijgen V-Sync uit in hun eigen configbestand.
- **Tweaks** (tabbladen Aanbevolen, Fortnite, Per game, Windows en Experimenteel): bovenaan **Grootste winst**: controles op scherm-Hz, games op de ingebouwde videokaart, accu/energiebesparing, geheugensnelheid (XMP/EXPO, alleen uitleg), videokaartdriver en zware achtergrond-apps. Daaronder kaarten met twee korte regels uitleg ("Wat doet het?" en "Wat merk je ervan?"), technische details achter "Meer info", en labels voor wanneer het werkt.
- **Pc of laptop**: nezty herkent zelf of je op een pc of een laptop zit (behuizing en batterij, een pc met UPS telt als pc). In Instellingen kun je het vastzetten. Tweaks die niet bij je apparaat passen krijgen een slotje met de reden erbij, en Game Boost slaat ze over. Op een laptop: energiemodus op beste prestaties, geen batterijbesparing tijdens het gamen, een melding als je op de accu speelt (optioneel: boost alleen met lader), controles voor hitte en terugschakeling, de snelle videokaart en MUX-switch, scherm-energiebesparing, wifi of kabel, en overbodige apps van de fabrikant. Op het dashboard staan batterij, lader, hitte en welke videokaart je scherm aanstuurt. nezty stuurt nooit ventilatoren of spanningen aan.
- **Muis & toetsenbord**: muisversnelling uit, snelste toetsherhaling, geen plaktoetsen-pop-ups, USB-energiebesparing uit, muis en toetsenbord nooit uitgezet om stroom te besparen, Alt+Shift wisselt geen taal meer, en tips voor NVIDIA Reflex en AMD Anti-Lag.
- **Controller**: Xbox-controllers (XInput) en PS4/PS5-controllers (HID): kabel of draadloos, batterij, rapportsnelheid met de wachttijd per knopdruk, een live knoppen- en trekkertest, een rustmeting voor sticks en trekkers met deadzone-advies, en een controlelijst "Zo reageren je knoppen het snelst". Alleen uitlezen: er gaat niets naar de controller.
- **Altijd als administrator**: nezty past Windows-instellingen aan en vraagt daarom bij het starten één keer om beheerdersrechten (Windows toont daarvoor de UAC-melding). Starten met Windows loopt via een geplande taak met de hoogste rechten.
- **Crosshair**: een doorklikbaar, altijd-bovenop venster met je eigen richtkruis (acht vormen, armen aan of uit, draaiing, kleur, lengte, dikte, opening, puntgrootte, rand, zichtbaarheid en positie), live voorbeeld, startpresets, eigen presets en een sneltoets. Er gaat niets de game in.
- **Achtergrond-apps**: één lijst met pictogram, naam, geheugen en processor, en de knop "Sluit alles wat niet nodig is". Systeem-, beveiligings- en anti-cheatprocessen staan er niet in en worden nooit gesloten. Ook je game wordt nooit gesloten.
- **Netwerk**: meet automatisch je internet en de game-regio's, geeft de beste regio het label "Aanbevolen" en zegt welke regio je in je game kiest.
- **Opstarten**, **Logboek** en **Instellingen** (taal, accentkleur, herstelpunt, rechten, SteamGridDB-sleutel).

## Veiligheid

- Vóór de eerste wijziging aan Windows maakt de app automatisch een herstelpunt. Dat geldt voor de boost, losse tweaks, de Fix-knoppen bij Grootste winst, opstartitems en achtergrondapps. Er zijn beheerdersrechten en Systeembeveiliging voor nodig. Lukt het niet, dan meldt nezty dat één keer per sessie en blijft alles via het logboek terug te zetten.
- Elke wijziging wordt gelogd met de oude waarde. Na elke wijziging controleert nezty of Windows hem echt heeft overgenomen.
- Staat iets al goed, dan doet nezty niets en toont het "Al goed ingesteld".
- Nooit: anti-cheat aanraken, iets in games injecteren, gamebestanden buiten de gebruikersconfig wijzigen, Defender, firewall of Windows Update uitzetten, je BIOS aanpassen of prioriteit "Realtime".

## Bouwen

Vereisten: Node.js 20 of nieuwer, Rust (stable) via rustup, en een C-toolchain. Met Visual Studio Build Tools werkt de MSVC-toolchain direct. Zonder beheerdersrechten kan het met de GNU-toolchain plus een portable MinGW-w64 (WinLibs) op het PATH.

```bash
npm install
```

```bash
.\scripts\dev.ps1
```

```bash
.\scripts\build.ps1
```

De installer en de losse exe komen in de map `release`. Ondertekenen voor Smart App Control: zie [ONDERTEKENEN.md](ONDERTEKENEN.md) en bouw met `.\scripts\build.ps1 -Sign`.

Met de GNU-toolchain heeft de losse exe `WebView2Loader.dll` in dezelfde map nodig. Het buildscript zet die ernaast en de installer regelt het zelf. Zonder die DLL stopt de exe direct met foutcode 0xC0000135.

Alleen de interface bekijken in een browser (met nepdata, raakt het systeem niet):

```bash
npm run dev
```

Zelftest voor ontwikkelaars (leest alle statussen en controles uit en schrijft een JSON-bestand, zonder iets te wijzigen):

```bash
nezty.exe --selftest C:\temp\nzty-selftest.json
```

De volledige app testen naast een al geopende nezty, zonder de echte instellingen of het logboek te raken: zet `NZTY_DATA_DIR` op een lege map en start met `--elevated`, dat de controle op een tweede venster overslaat. Zet `WEBVIEW2_USER_DATA_FOLDER` ook op een eigen map, zodat de twee vensters geen browsergegevens delen.

```powershell
$env:NZTY_DATA_DIR = "C:\temp\nzty-test\data"
$env:WEBVIEW2_USER_DATA_FOLDER = "C:\temp\nzty-test\wv"
.\release\nezty.exe --elevated --minimized
```

## Naam en gegevens

De app heet nezty (eerder nzty). De interne app-ID is bewust `com.nzty.desktop` gebleven: daaraan hangt de gegevensmap `%APPDATA%\com.nzty.desktop` met instellingen, logboek en covers. Zo houdt iedereen zijn gegevens en blijft "Alles terugzetten" werken. De installer verwijdert een oude nzty-installatie stil in updatemodus (gegevens blijven), ruimt de oude snelkoppelingen op en zet "starten met Windows" over. Zie `src-tauri/installer-hooks.nsh`.

## Updates uitbrengen

nezty zoekt bij het starten naar een nieuwe versie (uit te zetten in Instellingen). Een update wordt alleen geïnstalleerd als de handtekening klopt met de openbare sleutel in de app. De installer draait dan in updatemodus: gegevens blijven staan en nezty start daarna vanzelf opnieuw.

1. **Updatebron instellen (eenmalig).** Kopieer `scripts/update.voorbeeld.json` naar `scripts/update.json` en vul bij `baseUrl` de map in waar de updatebestanden online komen, bijvoorbeeld `https://github.com/JOUW-NAAM/nezty/releases/latest/download`. Deze bron wordt in de app ingebouwd, dus stel hem in vóór je de eerste versie deelt.
2. **Versie ophogen** in `src-tauri/tauri.conf.json` (`version`), bijvoorbeeld van 0.3.0 naar 0.3.1. Alleen een hoger nummer wordt als update gezien.
3. **Bouwen:** `.\scripts\build.ps1 -Notes "Wat er nieuw is"`.
4. **Online zetten:** upload alles uit `release\update` (de installer, het `.sig`-bestand en `latest.json`) naar de updatebron. Bij GitHub: maak een nieuwe release en sleep de drie bestanden erin.

De geheime updatesleutel staat in `nezty-update-sleutel` naast de broncode. **Maak er een back-up van.** Zonder die sleutel kunnen bestaande installaties nooit meer een update krijgen. Deel hem met niemand.

## Projectstructuur

- `src/` React-frontend. `src/lib/types.ts` is het contract met de backend, `src/lib/ipc.ts` de typed laag met nepdata voor de browser (`src/lib/mock.ts`). Teksten staan in `src/i18n/` (nl is de bron, en en fr moeten dezelfde sleutels hebben).
- `src-tauri/src/` Rust-backend:
  - `tweaks/` catalogus en implementaties (systeem, invoer, netwerk, per game, opruimen)
  - `bigwins.rs` en `display.rs` de controles voor Grootste winst
  - `journal.rs` logboek met terugzetlogica
  - `boost.rs`, `watcher.rs` en `fps.rs` Game Boost, automatisch boosten en FPS-meting
  - `covers.rs` en `icons.rs` game-afbeeldingen en programmapictogrammen
  - `crosshair.rs` het overlayvenster en de sneltoets
- `src-tauri/resources/` Intel PresentMon 2.6.0 (MIT-licentie, ondertekend door Intel).
- App-data (instellingen, profielen, logboek, afbeeldingen): `%APPDATA%\com.nzty.desktop\`.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

- **Committers and reviewers:** [nezty2](https://github.com/nezty2)
- **Approvers:** [nezty2](https://github.com/nezty2)

Every release is built from this repository by GitHub Actions and every signing request is approved manually by an approver. Only binaries built from this source are signed.

## Privacy policy

nezty does not collect, store or send any personal data. It has no accounts, no analytics and no tracking. nezty only makes these network connections:

- **Update check:** downloads `latest.json` and, when you click "Update now", the installer from this repository's GitHub Releases. Can be turned off in Settings > Updates.
- **Game images:** downloads cover images from Steam and Epic, and from SteamGridDB only if you enter your own API key. Can be turned off in Settings > Covers ("Fetch images online").
- **Network tests:** pings to well-known servers (for example 1.1.1.1) and game region servers, only when you open the Network page or the dashboard ping.
- **Bufferbloat test:** downloads and uploads test data (up to about 300 MB) to Cloudflare's speed test (speed.cloudflare.com), only when you start that test yourself on the Network page.
- **What uses your internet:** counts bytes per program on this PC for 3 seconds when you click "Measure now". Nothing is sent anywhere and the content of traffic is never read.

All tweaks are explained in the app before you apply them, are logged with their old value and can be undone with "Undo all". Uninstall nezty through Windows Settings > Apps.
