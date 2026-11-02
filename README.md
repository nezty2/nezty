# nezty

Windows-desktopapp (Windows 10 en 11) die games soepeler laat draaien met eerlijke, omkeerbare tweaks. Gebouwd met Tauri 2, React, TypeScript en Tailwind, met een Rust-backend die het Windows-register, PowerShell en Win32-API's aanspreekt. De app is beschikbaar in het Nederlands, Engels en Frans.

## Wat de app doet

- **Tweaks**: één scherm met bovenaan de status, daaronder de grote knop **Alles activeren** en een ingeklapt blok **Meer opties**. Er zijn geen tabbladen meer en ook geen apart Fortnite-tabblad: game-instellingen gelden voor alle games die nezty vindt. nezty toont alleen tweaks met een echt, meetbaar effect. Bij elke tweak staan twee korte regels uitleg ("Wat doet het?" en "Wat merk je ervan?"), technische details achter "Meer info", en wanneer hij werkt (meteen, na een herstart van de game of na een herstart van de pc).
- **Dashboard**: live processor, videokaart, geheugen, temperatuur (als er een sensor is), ping en een korte status (alles actief, nog niet alles actief of herstart nodig). De knop **Game Boost** past het gekozen profiel in één keer toe en toont daarna wat er echt gebeurd is. Draait de game al, dan meet nezty op verzoek de FPS vóór en na (Intel PresentMon).
- **Games**: nezty vindt games in Epic, Steam, Battle.net, Riot, EA, de Xbox-app en Minecraft, met de echte afbeelding van de game. Afbeeldingen komen eerst lokaal uit de launcher (Steam-bibliotheekcache, Xbox/Store-bestanden, de Epic-catalogus), daarna van de Steam-CDN en optioneel van SteamGridDB (eigen sleutel in Instellingen). Onder Meer opties > Games: per game een boostprofiel, automatisch boosten bij de start en terugzetten bij het sluiten, overlays, en voor Fortnite de FPS-limiet, de Performance-modus en een editor voor `GameUserSettings.ini` (altijd met back-up en per sleutel terug te zetten).
- **Pc of laptop**: nezty herkent zelf of je op een pc of een laptop zit (behuizing en batterij, een pc met UPS telt als pc). In Instellingen kun je het vastzetten. Alles voor laptops staat ook op een eigen pagina **Laptop** in de zijbalk. Tweaks die niet bij je apparaat passen krijgen een slotje met de reden erbij, en Alles activeren en Game Boost slaan ze over. Op de laptoppagina staan ook controles voor hitte en terugschakeling, de snelle videokaart en MUX-switch, wifi of kabel, en een FPS-rem van de videokaart op de accu. Op het dashboard staan batterij, lader, hitte en welke videokaart je scherm aanstuurt. nezty stuurt nooit ventilatoren of spanningen aan.
- **Licenties**: na de opstartanimatie log je in of maak je een account met een licentiesleutel (maand of demo, 1 apparaat). De server bepaalt of een sleutel geldig is. De app krijgt een ondertekend token en bewaart dat in Windows Credential Manager. Bovenin de zijbalk staat hoeveel tijd er nog over is. Verloopt de licentie, dan gaan tweaks, Game Boost, crosshair en metingen op slot, maar Alles terugzetten blijft altijd werken. Server en generator: zie [server/README.md](server/README.md). Zonder `scripts/license.json` bouwt nezty zonder licentie.
- **Muis & toetsenbord**: muisversnelling uit, geen plaktoetsen-pop-ups, Alt+Shift wisselt geen taal meer, USB blijft altijd aan (alleen pc), en tips voor NVIDIA Reflex en AMD Anti-Lag.
- **Controller**: Xbox-controllers (XInput) en PS4/PS5-controllers (HID): kabel of draadloos, batterij, rapportsnelheid met de wachttijd per knopdruk, een live knoppen- en trekkertest, een rustmeting voor sticks en trekkers met deadzone-advies, en een controlelijst "Zo reageren je knoppen het snelst". Alleen uitlezen: er gaat niets naar de controller.
- **Altijd als administrator**: nezty past Windows-instellingen aan en vraagt daarom bij het starten één keer om beheerdersrechten (Windows toont daarvoor de UAC-melding). Starten met Windows loopt via één geplande taak met de hoogste rechten.
- **Crosshair**: een doorklikbaar, altijd-bovenop venster met je eigen richtkruis (acht vormen, armen aan of uit, draaiing, kleur, lengte, dikte, opening, puntgrootte, rand, zichtbaarheid en positie), live voorbeeld, startpresets, eigen presets en een sneltoets. Er gaat niets de game in.
- **Achtergrond-apps**: één lijst met pictogram, naam, geheugen en processor, en de knop "Sluit alles wat niet nodig is". Systeem-, beveiligings- en anti-cheatprocessen staan er niet in en worden nooit gesloten. Ook je game wordt nooit gesloten.
- **Netwerk**: meet automatisch je internet en de game-regio's, geeft de beste regio het label "Aanbevolen" en zegt welke regio je in je game kiest.
- **Opstarten**, **Logboek** en **Instellingen** (taal, accentkleur, herstelpunt, rechten, SteamGridDB-sleutel). Opstarten toont geen items van Windows-beveiliging, systeem of anti-cheat en kan die dus ook niet uitzetten.

## Alles activeren

Eén klik zet alles aan wat op dit apparaat meetbaar helpt. Tijdens het werk toont een ring met animatie de voortgang en bij elke stap wat nezty nu doet. Een stap die niet lukt of niet past, wordt overgeslagen met de reden erbij. De rest gaat gewoon door. Na afloop staat er een overzicht: aangezet, stond al goed, overgeslagen en mislukt.

In deze volgorde:

1. **Herstelpunt** (één keer, vóór de eerste wijziging).
2. **Geen energiebesparingsplan**: haalt de pc van een besparend energieplan af.
3. **Scherm op de hoogste Hz**: elk scherm op de hoogste verversingssnelheid die het aankan.
4. **Windows-tweaks**: Windows Game Mode, Snellere games in een venster (Windows 11 22H2 of nieuwer), Muisversnelling uit, Geen plaktoetsen-pop-ups, Wifi-energiebesparing uit, Wifi zonder pingpieken. Op een laptop ook Energiemodus: beste prestaties en Processor op volle snelheid.
5. **Games op de snelle videokaart**: alleen op pc's en laptops met twee videokaarten.
6. **Fortnite**: V-Sync uit en NVIDIA Reflex aan (Reflex alleen met een NVIDIA-videokaart). Alleen als Fortnite dicht is.
7. **Andere games**: V-Sync uit in Counter-Strike 2 en Minecraft Java. Alleen als de game dicht is.

Daarna krijgen nieuwe games die nezty vindt dezelfde instellingen vanzelf. Een game die op dat moment draait, komt de volgende keer aan de beurt.

**Meer opties** (ingeklapt) bevat de losse tweaks om zelf te kiezen: Xbox Game Bar en opnemen uit, Energiebeheer op maximaal, Videokaart plant zelf (HAGS), Meldingen uit, USB blijft altijd aan, Alt+Shift wisselt geen taal meer, de laptoptweaks (Geen batterijbesparing tijdens het gamen, Processor reageert meteen, Games op de snelle kernen, USB wakker op de lader, Touchpad uit als er een muis is), de game-instellingen, dingen die je zelf moet controleren (geheugensnelheid, videokaartdriver, zware achtergrond-apps) en Tijdelijke bestanden opruimen.

**Uit de app gehaald in 0.5.0.** Twaalf tweaks hadden geen meetbaar effect of meer risico dan winst: timerresolutie, visuele effecten, snelle toetsherhaling, invoerapparaten niet uitzetten, updates niet delen, energiebesparing van de netwerkkaart, interrupt moderation, Nagle, network throttling, PCI Express-energiebeheer, volledig scherm-optimalisaties en hoge procesprioriteit. Bij de eerste start van 0.5.0 zet nezty elke wijziging daarvan die nog actief is precies terug via het logboek en meldt hoeveel dat er waren. Lukt dat zonder beheerdersrechten niet helemaal, dan probeert nezty het bij de volgende start opnieuw.

## Statuskleuren

- **Oranje** betekent maar één ding: **herstart je pc**. Een tweak die pas na een herstart werkt (nu alleen Videokaart plant zelf) staat aan, maar Windows is sinds die wijziging nog niet opnieuw opgestart. Bovenaan Tweaks staat dan de knop "Nu herstarten" (na een bevestiging) en in de zijbalk een oranje stip bij Tweaks. Staat Snel opstarten aan, dan telt alleen "Opnieuw opstarten" in het startmenu, niet "Afsluiten".
- **Groen** betekent klaar en actief.
- **Grijs** is al het andere: experimenteel, risico, beheerdersrechten nodig, uitleg, eerst de game opnieuw starten, of nog niet actief.

Na een echte herstart wordt het meteen groen. nezty zet bij zo'n tweak een vluchtige registersleutel onder `HKLM\SOFTWARE\nezty-pending-restart`. Windows houdt die alleen in het geheugen bij, dus na een herstart is hij weg. Wijzigingen van vóór 0.5.0 zonder zo'n sleutel vergelijkt nezty met de opstarttijd van Windows. Komt het venster weer in beeld, dan kijkt nezty de status opnieuw na.

## Tweaks blijven staan na een herstart

Tweaks staan in Windows zelf (register en energieplannen) en blijven na een herstart gewoon actief. nezty zet ze alleen terug als jij dat vraagt (losse tweak, logboek of Alles terugzetten), bij het verwijderen van de app, of als de licentie echt niet meer geldig is (zie hieronder). Uitgelogd zijn of minder dan 24 uur offline zet niets terug. Wifi zonder pingpieken werkt alleen zolang nezty draait. nezty probeert het elke 15 seconden opnieuw tot de wifi verbonden is, ook na slaapstand of wisselen van netwerk.

## Veiligheid

- Vóór de eerste wijziging aan Windows maakt de app automatisch een herstelpunt. Dat geldt voor Alles activeren, de boost, losse tweaks, de Fix-knoppen, opstartitems en achtergrondapps. Er zijn beheerdersrechten en Systeembeveiliging voor nodig. Lukt het niet, dan meldt nezty dat één keer per sessie en blijft alles via het logboek terug te zetten.
- Elke wijziging wordt gelogd met de oude waarde. Na elke wijziging controleert nezty of Windows hem echt heeft overgenomen.
- Staat iets al goed, dan doet nezty niets en toont het "Al goed ingesteld".
- Nooit: anti-cheat, Windows Defender, de Windows Firewall of Windows Update aanraken, iets in games injecteren, gamebestanden buiten de gebruikersconfig wijzigen, je BIOS aanpassen of de prioriteit van processen veranderen.

## Antivirus

Soms zet een virusscanner (meestal Microsoft Defender) nezty in quarantaine, vaak met een naam die eindigt op `!ml`. Dat is een vals alarm van een automatische inschatting. De belangrijkste reden: nezty is nog niet digitaal ondertekend (geen Authenticode). Een nieuw programma zonder handtekening dat beheerdersrechten vraagt, bij het inloggen start en Windows-instellingen aanpast, lijkt voor zo'n inschatting op iets verdachts. Elke nieuwe versie is bovendien een nieuw bestand dat nog niemand kent.

Wat nezty zelf doet om minder verdacht te lijken:

- Eén geplande taak in plaats van twee, die 45 seconden na het inloggen start en niet meteen tijdens de opstartscan van Windows. De taak heeft een auteur en een beschrijving, en het taakbestand komt niet meer in `%TEMP%` te staan.
- Bij het starten of inloggen draait er geen PowerShell meer, behalve eenmalig bij de eerste start van 0.5.0 als er nog oude netwerk- of apparaatwijzigingen van weggehaalde tweaks terug moeten. Netwerkkaarten en Minecraft worden via Windows-functies en het register uitgelezen. Verder draait PowerShell alleen nog bij een herstelpunt en bij het terugzetten van een paar oude wijzigingen.
- Updates worden gedownload in de eigen gegevensmap van nezty in plaats van `%TEMP%`, pas na controle van de handtekening.
- Links openen via Windows zelf, niet via `cmd.exe`.
- De zelftests zitten niet in een gewone release (zie Bouwen).

nezty zet nooit uitzonderingen in Defender en verandert niets aan je virusscanner.

**Vals alarm melden.** Meld het bestand bij Microsoft via [https://www.microsoft.com/en-us/wdsi/filesubmission](https://www.microsoft.com/en-us/wdsi/filesubmission). Kies "Software developer" als je de uitgever bent (anders "Home customer"), kies "Incorrectly detected as malware/malicious" en noem de naam van de melding uit Windows-beveiliging. Stuur de installer en `nezty.exe` in. Na goedkeuring werkt Microsoft de definities bij, meestal binnen een paar dagen. Doe dit voor elke nieuwe versie die gemeld wordt.

**Ondertekenen staat gepland.** Een ondertekende versie is de echte oplossing en geeft ook Smart App Control geen reden meer om nezty te blokkeren. Versie 0.5.0 is nog niet ondertekend. De mogelijke routes staan in [ONDERTEKENEN.md](ONDERTEKENEN.md).

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

Zelftest voor ontwikkelaars (leest alle statussen en controles uit, ook of de pc op een herstart wacht en wat Alles activeren nu zou doen, en schrijft een JSON-bestand, zonder iets te wijzigen). De zelftests (`--selftest`, `--selftest-apply`, `--selftest-license`, `--selftest-wifi`, `--selftest-device`, `--selftest-network`, `--selftest-roundtrip`) zitten alleen in een testbuild met de cargo-feature `selftest`. Een gewone release (`build.ps1`, `release.yml`) heeft ze niet. `--license-check` en `--restore-all` zitten wel altijd in de app.

```powershell
.\scripts\build.ps1 -Selftest            # of: npm run tauri build -- --features selftest
.\release\nezty.exe --selftest C:\temp\nzty-selftest.json
```

Breng zo'n testbuild nooit uit als update. `build.ps1 -Selftest` maakt daarom geen updatebestanden en zet een markering `release\SELFTEST` neer, waarop `publish.ps1` stopt.

De volledige app testen naast een al geopende nezty, zonder de echte instellingen of het logboek te raken: zet `NZTY_DATA_DIR` op een lege map en start met `--elevated`, dat de controle op een tweede venster overslaat. Zet `WEBVIEW2_USER_DATA_FOLDER` ook op een eigen map, zodat de twee vensters geen browsergegevens delen.

```powershell
$env:NZTY_DATA_DIR = "C:\temp\nzty-test\data"
$env:WEBVIEW2_USER_DATA_FOLDER = "C:\temp\nzty-test\wv"
.\release\nezty.exe --elevated --minimized
```

## Naam en gegevens

De app heet nezty (eerder nzty). De interne app-ID is bewust `com.nzty.desktop` gebleven: daaraan hangt de gegevensmap `%APPDATA%\com.nzty.desktop` met instellingen, logboek en covers. Zo houdt iedereen zijn gegevens en blijft "Alles terugzetten" werken. De installer verwijdert een oude nzty-installatie stil in updatemodus (gegevens blijven), ruimt de oude snelkoppelingen op en zet "starten met Windows" over. Zie `src-tauri/installer-hooks.nsh`.

**Installer.** Bestandsnaam `Nezty-Tweaks-Setup-<versie>.exe`, uitgever "Nezty", zonder links. De taal (NL, EN, FR) volgt Windows. Geen licentiepagina. De privacytekst staat in de app onder Instellingen > Over. Op de laatste pagina staan "Snelkoppeling op bureaublad" en "nezty starten" standaard aan. De afbeeldingen staan in `src-tauri/installer/`.

**Verwijderen = alles terug.** De uninstaller start eerst `nezty.exe --restore-all` (sluit nezty, zet elke gelogde wijziging terug, haalt de geplande taken, autostart en het bewaarde token weg) en wist daarna `%APPDATA%\com.nzty.desktop` en `%LOCALAPPDATA%\com.nzty.desktop`. Bij een update (`/UPDATE`) gebeurt dat niet. De uitkomst staat in `%TEMP%\nezty-restore-all.json`.

**Licentie niet meer geldig = alles terug.** Alleen als de server het zegt (verlopen, ingetrokken of verwijderd), of als het laatste token van de server ouder is dan zijn controlemoment (24 uur zonder bevestiging, bij een demo 5 minuten). Dan stopt nezty Game Boost en de crosshair en zet alle tweaks terug. Uitgelogd zijn, "Ingelogd blijven" uit of minder dan 24 uur offline zet niets terug: tweaks blijven staan na een herstart van de pc. Daarvoor bewaart nezty het laatste ondertekende token altijd in Credential Manager (`nezty/license-last`), de sessie alleen met "Ingelogd blijven". De precieze regel staat bovenaan `src-tauri/src/license.rs`. De controle draait bij het starten, elke 15 minuten en 45 seconden na het inloggen in Windows. Staat starten met Windows aan, dan doet de taak "nezty autostart" dat. Staat het uit, dan is er alleen de stille taak "nezty licentiecontrole" (`nezty.exe --license-check`, uitkomst in `%TEMP%\nezty-license-check.json`). Er staat dus altijd hooguit één taak. Zie `server/README.md`.

**Updates en instellingen.** Een update wordt gedownload naar `%APPDATA%\com.nzty.desktop\updates\Nezty-Tweaks-Setup.exe` (niet naar %TEMP%) en bij de volgende start opgeruimd. `settings.json`, `profiles.json` en het logboek worden eerst naar een tijdelijk bestand geschreven en dan in één keer vervangen. Van `settings.json` en `profiles.json` staat de vorige versie in `.bak`, die nezty gebruikt als het hoofdbestand onleesbaar is.

## Updates uitbrengen

nezty zoekt bij het starten en daarna elke 30 minuten naar een nieuwe versie, ook op het inlogscherm en als de licentie verlopen of ingetrokken is. Een update wordt alleen geïnstalleerd als de handtekening klopt met de openbare sleutel in de app. De installer draait dan in updatemodus: gegevens blijven staan, nezty start daarna vanzelf opnieuw en meldt kort "nezty is bijgewerkt naar versie X".

De updatebestanden staan in Supabase Storage (bucket `updates`, openbaar), niet op GitHub. De repo blijft privé.

**Een update uitbrengen in drie stappen:**

1. **Versie ophogen** in `src-tauri/tauri.conf.json`, `package.json` en `src-tauri/Cargo.toml` (`version`), bijvoorbeeld van 0.5.0 naar 0.5.1. Alleen een hoger nummer wordt als update gezien.
2. **Committen en pushen naar `main`.** Het commitbericht is de tekst die gebruikers bij de update zien, dus schrijf bijvoorbeeld `nezty 0.5.1: crosshair onthoudt zijn kleur`.
3. **Klaar.** GitHub Actions (`release.yml`) bouwt de app, ondertekent de update en zet `Nezty-Tweaks-Setup-0.5.1.exe`, het `.sig`-bestand en `latest.json` online. Kijk onder **Actions** of de run groen is. Klanten krijgen de update binnen een half uur.

Eenmalig instellen staat bovenin `.github/workflows/release.yml`: twee variabelen (`SUPABASE_PROJECT_REF`, `NEZTY_LICENSE_PUBKEY`) en twee geheimen (`SUPABASE_SERVICE_ROLE_KEY`, `TAURI_SIGNING_PRIVATE_KEY`). Zonder die geheimen stopt de workflow met een duidelijke melding en gaat er niets online.

Lokaal bouwen en publiceren kan ook: `.\scripts\release.ps1 -Version 0.5.1 -Notes "Wat er nieuw is"` (vereist `scripts\update.json` en `scripts\publish.json`, zie de voorbeeldbestanden).

De geheime updatesleutel staat in `nezty-update-sleutel` naast de broncode. **Maak er een back-up van.** Zonder die sleutel kunnen bestaande installaties nooit meer een update krijgen. Deel hem met niemand.

## Projectstructuur

- `src/` React-frontend. `src/lib/types.ts` is het contract met de backend, `src/lib/ipc.ts` de typed laag met nepdata voor de browser (`src/lib/mock.ts`). Teksten staan in `src/i18n/` (nl is de bron, en en fr moeten dezelfde sleutels hebben).
- `src-tauri/src/` Rust-backend:
  - `tweaks/` catalogus en implementaties (systeem, invoer, netwerk, laptop, per game, opruimen). `tweaks::REMOVED` noemt de tweaks die in 0.5.0 zijn verdwenen en bij de eerste start worden teruggezet
  - `activate.rs` Alles activeren (ook voor nieuwe games), `restart.rs` herkennen van een herstart en de pc herstarten
  - `bigwins.rs` en `display.rs` de controles voor Grootste winst
  - `journal.rs` logboek met terugzetlogica
  - `boost.rs`, `watcher.rs` en `fps.rs` Game Boost, automatisch boosten en FPS-meting
  - `covers.rs` en `icons.rs` game-afbeeldingen en programmapictogrammen
  - `crosshair.rs` het overlayvenster en de sneltoets
  - `license.rs` licentiecontrole en terugzetten, `autostart.rs` geplande taken, `updater.rs` updates
- `src-tauri/installer-hooks.nsh` extra stappen van de installer en uninstaller, `src-tauri/installer/` afbeeldingen
- `scripts/` bouwen (`build.ps1`), publiceren (`publish.ps1`, `upload-supabase.ps1`) en de licentietest (`test-licentie.ps1`)
- `.github/workflows/release.yml` automatische update bij een push naar main
- `src-tauri/resources/` Intel PresentMon 2.6.0 (MIT-licentie, ondertekend door Intel).
- App-data (instellingen, profielen, logboek, afbeeldingen): `%APPDATA%\com.nzty.desktop\`.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

- **Committers and reviewers:** [nezty2](https://github.com/nezty2)
- **Approvers:** [nezty2](https://github.com/nezty2)

Every release is built from this repository by GitHub Actions and every signing request is approved manually by an approver. Only binaries built from this source are signed. Signing is planned but not active yet: version 0.5.0 is not signed.

## Privacy policy

nezty has no analytics and no tracking. To use a license you create an account on the Nezty license server: a username, an optional e-mail address and your password, stored only as a PBKDF2 hash. The server also keeps your license key as a hash and a device id (a hash of the Windows MachineGuid, never the real value). Nothing else about you or your PC is collected. nezty only makes these network connections:

- **Update check:** downloads `latest.json` and the installer from the Nezty update server (Supabase Storage). Updates are mandatory so that every installation has the version that cleans up correctly.
- **License check:** sends a device id (a hash of the Windows MachineGuid, never the real value) and the session to the Nezty license server, at startup and every 15 minutes. No other data is sent.
- **Game images:** downloads cover images from Steam and Epic, and from SteamGridDB only if you enter your own API key. Can be turned off in Settings > Covers ("Fetch images online").
- **Network tests:** pings to well-known servers (for example 1.1.1.1) and game region servers, only when you open the Network page or the dashboard ping.
- **Bufferbloat test:** downloads and uploads test data (up to about 300 MB) to Cloudflare's speed test (speed.cloudflare.com), only when you start that test yourself on the Network page.
- **What uses your internet:** counts bytes per program on this PC for 3 seconds when you click "Measure now". Nothing is sent anywhere and the content of traffic is never read.

All tweaks are explained in the app before you apply them, are logged with their old value and can be undone with "Undo all". When the license expires or is revoked, nezty puts everything back by itself. Uninstalling nezty through Windows Settings > Apps also puts everything back first and then removes all nezty data.
