import type { Messages } from './types';

/** German strings of the online part libraries (part of `de`). */
export const partsDe: Messages['parts'] = {
  title: 'Online-Bauteilbibliotheken',
  open: 'Online-Bauteile',
  openTip: 'Bauteile auf SnapEDA und Ultra Librarian finden und ihre Downloads importieren',
  intro:
    'Ein Bauteil auf SnapEDA oder Ultra Librarian suchen und im KiCad-Format (KiCad 6 oder neuer) herunterladen. ElectronicWerk importiert den Download in die Projektbibliothek: Symbol, Footprint und 3D-Modell.',
  query: 'Teilenummer oder Suchbegriff',
  queryPlaceholder: 'zum Beispiel NE555 oder TPS62130',
  search: 'Auf {provider} suchen',
  searchExternal: 'Im eigenen Browser',
  searchExternalTip:
    '{provider} im eigenen Browser öffnen; den Download mit „Download importieren…“ übernehmen',
  searchOnline: 'Online suchen (SnapEDA, Ultra Librarian)…',
  importFile: 'Download importieren…',
  importFileTip:
    'Eine heruntergeladene ZIP-Datei (oder eine KiCad-Bibliothek oder STEP-Datei) importieren',
  providers: {
    snapeda:
      'Millionen Symbole, Footprints und 3D-Modelle, kostenlos mit Konto. Dateien unter CC BY-SA 4.0 mit Design-Ausnahme.',
    ultralibrarian:
      'Modelle der Hersteller in vielen CAD-Formaten, kostenlos mit Konto. Beim Herunterladen „KiCad“ wählen.',
  },
  session: 'Angemeldet (das Anbieterfenster behält die Sitzung)',
  noSession: 'Nicht angemeldet',
  howItWorks:
    'Das Anbieterfenster ist ein eigener, abgeschotteter Browser: ElectronicWerk sieht Ihr Passwort nie und sucht oder lädt nichts von selbst. Was Sie dort herunterladen, wird zum Import angeboten.',
  noProject: 'Öffnen Sie ein Projekt, um {file} zu importieren.',
  downloaded: '{file} heruntergeladen',
  downloadFailed: 'Der Download von {file} ist fehlgeschlagen.',
  notImportable: '{file} ist kein KiCad-Download; die Datei liegt in {folder}.',
  settings: {
    title: 'Online-Bauteilbibliotheken',
    signIn: 'Anmelden',
    signOut: 'Abmelden',
    signOutTip: 'Löscht die Anmeldung und alles, was die Website gespeichert hat',
    terms: 'Nutzungsbedingungen',
    signedOut: 'Von {provider} abgemeldet',
  },
  dialog: {
    title: 'Bauteil-Download importieren',
    reading: '{file} wird gelesen…',
    from: 'Von {provider}',
    unknownSource: 'Aus unbekannter Quelle',
    nickname: 'Bibliotheksname',
    nicknameHint: 'Das Präfix der importierten Namen, zum Beispiel {example}.',
    target: 'In',
    symbols: 'Symbole',
    footprints: 'Footprints',
    models: '3D-Modelle',
    modelsHint: 'Werden nach {folder} im Projektordner kopiert.',
    pins: '{count} Pins',
    pads: '{count} Pads',
    exists: 'schon in der Bibliothek (übersprungen)',
    duplicate: {
      exact: 'identisch mit {name} in der Basisbibliothek',
      pads: 'gleiche Pads wie {name} in der Basisbibliothek',
      near: 'Pads höchstens 0,05 mm von {name} in der Basisbibliothek entfernt',
      pins: 'gleiche Pins wie {name} in der Basisbibliothek',
    },
    useLibrary: 'Stattdessen {name} aus der Basisbibliothek verwenden',
    unused: 'von keinem Footprint verwendet',
    noModel: 'kein 3D-Modell',
    problems: 'Probleme',
    nothing: 'Dieser Download enthält nichts zum Importieren.',
    import: 'Importieren',
    cancel: 'Abbrechen',
    imported:
      '{symbols} Symbole, {footprints} Footprints und {models} 3D-Modelle in {library} importiert',
  },
};
