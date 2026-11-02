# nezty licentieserver

De licentiecontrole draait op een server. De app vertrouwt alleen wat de server ondertekent.

## Waarom Supabase

- **Gratis om mee te beginnen.** Een database (Postgres) met een dashboard waarin je alles kunt bekijken, plus "Edge Functions" voor de logica. Geen eigen server die je moet bijhouden of updaten.
- **Geheimen blijven op de server.** De privésleutel voor tokens, het geheim om sleutels te hashen en de hash van je beheerderssleutel staan als "secrets" in Supabase. De app krijgt alleen een publieke sleutel.
- **Eén functie.** Alle logica zit in `supabase/functions/_shared/core.ts`. Dezelfde code draait ook in de lokale testserver en in de tests.

Let op: een gratis Supabase-project wordt na een week zonder verkeer gepauzeerd. Met actieve klanten (de app controleert elke 15 minuten) gebeurt dat niet. Wil je zekerheid, neem dan het Pro-abonnement.

## Wat wordt opgeslagen

| Tabel | Inhoud |
|---|---|
| `licenses` | type, looptijd, activatie- en einddatum, apparaat, alleen een **hash** van de sleutel plus de laatste 4 tekens |
| `users` | gebruikersnaam, e-mail (optioneel), wachtwoord alleen als **PBKDF2-hash** (600.000 rondes) |
| `sessions` | alleen een **hash** van het sessiegeheim |
| `demo_devices` | apparaten die de gratis demo al gebruikten |
| `settings` | demo aan/uit en je contactgegevens |

Alle tabellen hebben Row Level Security aan zonder policies: alleen de serverfunctie kan erbij.

## Eenmalig instellen

Alle commando's vanuit de map `nzty-broncode`.

1. **Account en project.** Maak op supabase.com een account en een nieuw project. Noteer de project-ref (in de url: `https://<project-ref>.supabase.co`).
2. **Tabellen.** Open in Supabase de **SQL Editor**, plak de inhoud van `server/supabase/migrations/20261029000000_licenses.sql` en klik Run.
3. **Geheimen maken.**
   ```
   npm run licentie:sleutels-maken
   ```
   Dit toont drie groepen. Bewaar ze veilig (bijvoorbeeld in je wachtwoordmanager). Ze worden maar één keer getoond.
4. **Supabase CLI koppelen** (in de map `server`):
   ```
   cd server
   npx supabase login
   npx supabase link --project-ref <project-ref>
   ```
5. **Geheimen op de server zetten** (groep 1 uit stap 3):
   ```
   npx supabase secrets set LICENSE_SIGNING_KEY=... LICENSE_KEY_PEPPER=... ADMIN_KEY_SHA256=...
   ```
6. **De functie uploaden:**
   ```
   npx supabase functions deploy license --no-verify-jwt
   ```
   Het adres van je server is dan `https://<project-ref>.supabase.co/functions/v1/license`.
7. **Generator op je eigen pc** (groep 2 uit stap 3). In PowerShell, eenmalig:
   ```
   setx NEZTY_LICENSE_URL "https://<project-ref>.supabase.co/functions/v1/license"
   setx NEZTY_ADMIN_KEY "nzadm_..."
   ```
   Open daarna een nieuw venster.
8. **Contact en demo:**
   ```
   npm run licentie:instellingen -- --discord jouwnaam --email jij@voorbeeld.nl --demo aan
   ```
9. **De app koppelen.** Maak `scripts/license.json` (staat niet in de repo), met de publieke sleutel uit groep 3:
   ```json
   { "url": "https://<project-ref>.supabase.co/functions/v1/license", "publicKey": "..." }
   ```
   Vanaf nu bouwt `scripts/build.ps1` nezty met inlogscherm. Zonder dit bestand bouwt hij zonder licentie, zoals voorheen.

## Dagelijks gebruik

```
npm run licentie:maak -- --type maand
npm run licentie:maak -- --type demo
npm run licentie:maak -- --type maand --aantal 5
npm run licentie:verleng -- --sleutel nezty-xxxx-xxxx-xxx --dagen 30
npm run licentie:intrekken -- --sleutel nezty-...
npm run licentie:verwijder -- --sleutel nezty-...                (sleutel, sessie en account helemaal weg)
npm run licentie:reset-apparaat -- --sleutel nezty-...     (klant heeft een nieuwe pc)
npm run licentie:lijst
npm run licentie:info -- --sleutel nezty-...
npm run gebruiker:reset-wachtwoord -- --gebruiker naam
npm run licentie:instellingen -- --demo uit
```

In plaats van `--sleutel` kun je `--id` gebruiken (de eerste tekens uit `licentie:lijst`), handig als je een sleutel kwijt bent.

## Hoe het werkt

- **Sleutels.** Bijvoorbeeld `nezty-8273-ctaa-871`: 10 willekeurige tekens (50 bits, ruim duizend biljoen mogelijkheden) plus een controleteken. Kleine letters en cijfers zonder 0, 1, i en o. Hoofdletters of spaties bij het overtypen maken niet uit, en een typfout ziet de app al tijdens het typen.
- **Tijd.** De tijd gaat lopen bij het activeren, niet bij het maken. Maand = 30 dagen, demo = 2 uur. Verlengen telt door vanaf de einddatum, of vanaf nu als de licentie al verlopen was.
- **Eén apparaat.** De app stuurt een apparaat-ID (een hash van de Windows MachineGuid). Inloggen op een ander apparaat kan pas na `licentie:reset-apparaat`.
- **Token.** De server ondertekent de uitkomst met Ed25519. De app controleert de handtekening en bewaart sessie en token in Windows Credential Manager (alleen met "Ingelogd blijven"). Het laatste token bewaart de app altijd apart (`nezty/license-last`), ook zonder "Ingelogd blijven" en na uitloggen. Daarmee weet de app na een herstart wat de server het laatst zei.
- **Offline.** Een maandlicentie werkt zonder internet nog 24 uur, een demo alleen online.
- **Intrekken, verwijderen en verlengen.** Dit ziet de klant bij de volgende controle: bij het starten, daarna elke 15 minuten, en bij de demo elke minuut. Zegt de server dat de licentie niet meer geldig is (ingetrokken, verlopen of verwijderd), of is het laatste token ouder dan zijn controlemoment (24 uur zonder bevestiging, demo 5 minuten), dan zet de app meteen alle tweaks terug naar hoe het stond (op basis van het herstellogboek), ook wat "Alles activeren" had aangezet, stopt Game Boost en de crosshair en toont "Licentie ingetrokken" of "Licentie verlopen" met je contactgegevens.
- **Verwijderen** (`licentie:verwijder`) haalt sleutel, sessie en account weg. Voor de klant is dat hetzelfde als intrekken. Wil hij verder, dan heeft hij een nieuwe sleutel nodig en maakt hij opnieuw een account (dezelfde gebruikersnaam mag weer).
- **Niet terugzetten.** Alleen uitgelogd, "Ingelogd blijven" uit of minder dan 24 uur offline: de tweaks blijven staan, ook na een herstart van de pc. De app toont wel het inlogscherm. Zonder enig bewaard token (bijvoorbeeld na een update vanaf 0.4.0 zonder "Ingelogd blijven") krijgt de klant 24 uur om in te loggen.
- **Ook zonder de app te openen.** 45 seconden na het inloggen in Windows controleert de app de licentie. Staat starten met Windows aan, dan doet de taak "nezty autostart" dat. Staat het uit, dan maakt de app de stille taak "nezty licentiecontrole", die alleen de licentie controleert en zo nodig alles terugzet. Er is altijd hooguit één van de twee.

## Testen zonder Supabase

```
npm run licentie:test
npm run licentie:sleutels-maken -- --bestand C:\pad\naar\test-secrets.json
npm run licentie:testserver -- --keys C:\pad\naar\test-secrets.json --data C:\pad\naar\test-data.json
```

De testserver draait op `http://127.0.0.1:8787/license` met precies dezelfde logica.
