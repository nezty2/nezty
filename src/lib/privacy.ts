/** Laat je Windows-gebruikersnaam niet zien in paden (handig als je streamt of een screenshot deelt). */
export function prettyPath(p: string): string {
  return p.replace(/^[a-z]:[\\/]users[\\/][^\\/]+/i, "%USERPROFILE%");
}
