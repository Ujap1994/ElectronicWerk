/**
 * Requests to open the library from elsewhere (the schematic's place
 * dialog opens the part catalog with what was typed there).
 */
import { Store } from '../schematic/stores';

/** Open the library at the part catalog, searching for `text`. */
export const catalogRequest = new Store<{ readonly text: string } | null>(null);

export function openCatalog(text = ''): void {
  catalogRequest.set({ text });
}
