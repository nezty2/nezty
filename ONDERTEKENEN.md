# nezty ondertekenen

Windows Smart App Control blokkeert programma's zonder geldige digitale handtekening. Volgens Microsoft laat Smart App Control een app wel toe als die is ondertekend met een code-signing-certificaat van een vertrouwde uitgever. Een zelfgemaakt certificaat telt niet. Dit lost het probleem op voor jou én voor iedereen die nezty installeert.

Bronnen: [Sign your app for Smart App Control compliance](https://learn.microsoft.com/windows/apps/develop/smart-app-control/code-signing-for-smart-app-control) en [Code signing options for Windows app developers](https://learn.microsoft.com/windows/apps/package-and-deploy/code-signing-options) (gecontroleerd op 29 oktober 2026).

## Welke route past bij jou

| Route | Voor wie | Kosten |
|---|---|---|
| **OV-certificaat** van een certificaatuitgever | Particulieren in Nederland (aanbevolen) | Ongeveer $150 tot $300 per jaar |
| **SignPath Foundation** | Alleen als de broncode van nezty openbaar wordt (open source) | Gratis |
| **Azure Artifact Signing** | Organisaties in de EU. Particulieren alleen in de VS en Canada | Ongeveer $9,99 per maand |
| **Microsoft Store (MSIX)** | Apps die in de Store passen | Gratis ondertekening, maar zie hieronder |

De Microsoft Store past slecht bij nezty: verpakte apps krijgen een afgeschermde kopie van een deel van het register, waardoor tweaks niet echt in Windows terechtkomen, en updates lopen dan via de Store in plaats van via de eigen updater.

## Route 1: OV-certificaat (aanbevolen)

1. **Kies een uitgever** die in het Microsoft Trusted Root Program zit, bijvoorbeeld Certum, Sectigo, SSL.com of DigiCert. Kies een variant waarbij de sleutel in de cloud staat (bijvoorbeeld Certum SimplySign) of op een USB-token. Sinds 2023 mag de sleutel niet meer als los bestand worden geleverd.
2. **Laat je identiteit controleren.** De uitgever vraagt een geldig identiteitsbewijs en soms een adresbewijs. Dit duurt meestal een paar werkdagen.
3. **Installeer de software van de uitgever.** Daarmee verschijnt het certificaat in Windows onder Certificaten, Huidige gebruiker, Persoonlijk.
4. **Zoek de thumbprint op** in PowerShell:

```bash
Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert | Format-List Subject, Thumbprint, NotAfter
```

5. **Maak het instellingenbestand.** Kopieer `scripts\signing.example.json` naar `scripts\signing.json`, zet `"method": "certstore"` en plak de thumbprint. Dit bestand blijft op je pc.
6. **Breng een ondertekende versie uit:**

```bash
.\scripts\release.ps1 -Version 0.3.2 -Notes "nezty is nu ondertekend" -Sign
```

Na afloop toont het script per bestand de status. `Valid` betekent dat Windows de handtekening vertrouwt.

## Route 2: SignPath Foundation (gratis, alleen open source)

SignPath ondertekent gratis voor openbare open-sourceprojecten. Daarvoor moet de broncode van nezty openbaar op GitHub staan en wordt er via GitHub Actions gebouwd. Bekijk de voorwaarden op [signpath.io](https://signpath.io).

## Wat je daarna kunt verwachten

- **Smart App Control** laat een geldig ondertekende nezty starten, ook op de pc's van je gebruikers.
- **SmartScreen** kan in het begin nog "Onbekende app" tonen. Een nieuwe uitgever bouwt vertrouwen op naarmate meer mensen de app installeren. Onderteken elke nieuwe versie met hetzelfde certificaat, dan blijft dat vertrouwen behouden.
- **Updates** blijven werken zoals nu: de installer krijgt de Authenticode-handtekening en daarna de updatehandtekening.

## Wat er in de build zit

- `scripts\sign.ps1` ondertekent één bestand met SHA-256 en een tijdstempel. Dat stempel houdt de handtekening geldig nadat het certificaat verloopt.
- `scripts\build.ps1 -Sign` laat Tauri de app-exe, de DLL's, de NSIS-onderdelen en de installer ondertekenen. Daarna ondertekent het script ook de losse `nezty.exe` in `release`.
- `scripts\release.ps1 -Sign` doet hetzelfde en zet de update daarna online.
