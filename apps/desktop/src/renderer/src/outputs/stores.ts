import { Store } from '../schematic/stores';

/** Output commands of the open project (for the command palette). */
export interface OutputCommands {
  /** Opens the first output job (creating one if there is none). */
  readonly openJob: () => void;
  /** Asks for Gerber and drill files and shows them in the Gerber viewer. */
  readonly openGerbers: () => void;
}

export const outputCommands = new Store<OutputCommands | null>(null);
