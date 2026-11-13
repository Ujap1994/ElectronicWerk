/**
 * State of the online part libraries: whether their dialog is open and which
 * downloaded file waits for import.
 */
import { Store } from '../schematic/stores';

/** The online part libraries dialog is open (with a query to start from). */
export const onlineParts = new Store<{ readonly query: string } | null>(null);

/** A downloaded file whose import dialog is shown. */
export const pendingImport = new Store<string | null>(null);

export function openOnlineParts(query = ''): void {
  onlineParts.set({ query });
}
