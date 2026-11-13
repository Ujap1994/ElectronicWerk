import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import {
  BUILTIN_VARIABLES,
  formatLength,
  formatSize,
  isVariableName,
  UNITS,
  type DocumentSummary,
  type FileEntry,
  type ProjectSnapshot,
  type Units,
} from '@shared/project';

import { useProject } from '../project/ProjectProvider';
import type { GroupId, TreeTarget } from '../project/tree';
import { useAppState } from '../state';
import { isViewable, useOpenDocument } from '../viewer/open-context';
import { Icon } from './Icon';

// --- Small building blocks --------------------------------------------------

/** A text field that commits on Enter or when it loses focus. */
function CommitField({
  label,
  value,
  multiline = false,
  testId,
  onCommit,
}: {
  readonly label: string;
  readonly value: string;
  readonly multiline?: boolean;
  readonly testId: string;
  readonly onCommit: (value: string) => void;
}): ReactNode {
  const [draft, setDraft] = useState(value);
  const commit = (): void => {
    if (draft !== value) {
      onCommit(draft);
    }
  };
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      {multiline ? (
        <textarea
          className="field__control"
          rows={3}
          value={draft}
          data-testid={testId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={commit}
        />
      ) : (
        <input
          className="field__control"
          type="text"
          value={draft}
          data-testid={testId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              commit();
            } else if (event.key === 'Escape') {
              setDraft(value);
            }
          }}
        />
      )}
    </label>
  );
}

function Section({
  title,
  children,
  testId,
}: {
  readonly title: string;
  readonly children: ReactNode;
  readonly testId?: string;
}): ReactNode {
  return (
    <section className="panel" data-testid={testId}>
      <h3 className="panel__title">{title}</h3>
      {children}
    </section>
  );
}

function Stats({
  items,
}: {
  readonly items: readonly (readonly [MessageKey, string | number | undefined])[];
}): ReactNode {
  const { t } = useAppState();
  return (
    <dl className="stats">
      {items
        .filter(([, value]) => value !== undefined)
        .map(([label, value]) => (
          <div key={label} className="stats__item">
            <dt>{t(label)}</dt>
            <dd>{value}</dd>
          </div>
        ))}
    </dl>
  );
}

function Coming({ text }: { readonly text: MessageKey }): ReactNode {
  const { t } = useAppState();
  return (
    <p className="callout callout--info">
      <Icon name="info" />
      <span>{t(text)}</span>
    </p>
  );
}

/** Loads `document.properties` and reloads after every project change. */
function useDocumentProps(document: DocumentSummary): Record<string, unknown> | null {
  const { project, documentProperties } = useProject();
  const [props, setProps] = useState<Record<string, unknown> | null>(null);
  useEffect(() => {
    let cancelled = false;
    void documentProperties(document.id).then((result) => {
      if (!cancelled) {
        setProps(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [document.id, documentProperties, project]);
  return props;
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function num(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

/** Formats lengths in the project's units and the UI language. */
function useLength(): (nm: number) => string {
  const { units, language } = useUnits();
  return (nm) => formatLength(nm, units, language);
}

/** The project's units and the UI language (for number formatting). */
function useUnits(): { readonly units: Units; readonly language: string } {
  const { project } = useProject();
  const { snapshot } = useAppState();
  return { units: project?.settings.units ?? 'mm', language: snapshot.language };
}

function DocumentHeader({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  const { renameDocument, showInFolder } = useProject();
  const open = useOpenDocument();
  return (
    <>
      {open !== null && isViewable(document) && (
        <button
          type="button"
          className="button button--primary details__open"
          data-testid="details-open"
          onClick={() => {
            open(document);
          }}
        >
          <Icon name="eye" />
          <span>{t('viewer.open')}</span>
        </button>
      )}
      <CommitField
        key={document.name}
        label={t('details.name')}
        value={document.name}
        testId="document-name"
        onCommit={(name) => {
          void renameDocument(document.id, name);
        }}
      />
      <div className="field">
        <span className="field__label">{t('details.file')}</span>
        <div className="field__row">
          <code className="path">{document.path}</code>
          <button
            type="button"
            className="button button--small"
            onClick={() => {
              showInFolder(document.path);
            }}
          >
            {t('project.showInFolder')}
          </button>
        </div>
      </div>
    </>
  );
}

// --- Project ------------------------------------------------------------------

function VariablesEditor({ project }: { readonly project: ProjectSnapshot }): ReactNode {
  const { t } = useAppState();
  const { setProperties } = useProject();
  const [name, setName] = useState('');
  const [value, setValue] = useState('');
  const entries = Object.entries(project.variables);
  const trimmed = name.trim();
  const problem =
    trimmed === ''
      ? null
      : !isVariableName(trimmed)
        ? t('details.invalidVariable')
        : (BUILTIN_VARIABLES as readonly string[]).includes(trimmed)
          ? t('details.builtinVariable')
          : trimmed in project.variables
            ? t('details.duplicateVariable')
            : null;

  const add = (): void => {
    if (trimmed === '' || problem !== null) {
      return;
    }
    const added = { name: trimmed, value };
    // Clear at once so the next variable can be typed right away; restore the
    // input if the engine rejects the change.
    setName('');
    setValue('');
    void setProperties({ variables: { [added.name]: added.value } }).then((ok) => {
      if (!ok) {
        setName(added.name);
        setValue(added.value);
      }
    });
  };

  return (
    <Section title={t('details.variables')} testId="variables">
      <p className="field__hint">
        {t('details.variablesHint', { builtins: BUILTIN_VARIABLES.join(', ') })}
      </p>
      <table className="table">
        <thead>
          <tr>
            <th>{t('details.variableName')}</th>
            <th>{t('details.variableValue')}</th>
            <th aria-hidden="true" />
          </tr>
        </thead>
        <tbody>
          {entries.map(([key, current]) => (
            <tr key={key} data-testid={`variable-${key}`}>
              <td>
                <code>{key}</code>
              </td>
              <td>
                <input
                  key={current}
                  className="field__control field__control--table"
                  defaultValue={current}
                  aria-label={key}
                  onBlur={(event) => {
                    if (event.target.value !== current) {
                      void setProperties({ variables: { [key]: event.target.value } });
                    }
                  }}
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') {
                      event.currentTarget.blur();
                    }
                  }}
                />
              </td>
              <td>
                <button
                  type="button"
                  className="icon-button"
                  aria-label={t('details.removeVariable', { name: key })}
                  title={t('details.removeVariable', { name: key })}
                  onClick={() => {
                    void setProperties({ variables: { [key]: null } });
                  }}
                >
                  <Icon name="trash" size={16} />
                </button>
              </td>
            </tr>
          ))}
          <tr>
            <td>
              <input
                className="field__control field__control--table"
                value={name}
                placeholder={t('details.variableName')}
                aria-label={t('details.variableName')}
                aria-invalid={problem !== null}
                data-testid="variable-new-name"
                onChange={(event) => {
                  setName(event.target.value.toUpperCase());
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    add();
                  }
                }}
              />
            </td>
            <td>
              <input
                className="field__control field__control--table"
                value={value}
                placeholder={t('details.variableValue')}
                aria-label={t('details.variableValue')}
                data-testid="variable-new-value"
                onChange={(event) => {
                  setValue(event.target.value);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    add();
                  }
                }}
              />
            </td>
            <td>
              <button
                type="button"
                className="icon-button"
                aria-label={t('details.addVariable')}
                title={t('details.addVariable')}
                disabled={trimmed === '' || problem !== null}
                data-testid="variable-add"
                onClick={add}
              >
                <Icon name="plus" size={16} />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      {problem !== null && <p className="field__error">{problem}</p>}
    </Section>
  );
}

function ProjectOverview({ project }: { readonly project: ProjectSnapshot }): ReactNode {
  const { t } = useAppState();
  const { setProperties, showInFolder } = useProject();
  const length = useLength();
  const { units, language } = useUnits();
  const sum = (pick: (document: DocumentSummary) => number | undefined): number =>
    project.documents.reduce((total, document) => total + (pick(document) ?? 0), 0);
  const board = project.documents.find((document) => document.kind === 'pcb');
  const boardSize = board?.stats.boardSize;
  return (
    <div className="details__body" data-testid="details-project">
      {project.needsUpgrade && (
        <p className="callout callout--info">
          <Icon name="info" />
          <span>{t('project.needsUpgrade')}</span>
        </p>
      )}
      {project.journalError !== null && (
        <p className="callout callout--warning">
          <Icon name="warning" />
          <span>{t('project.journalError', { error: project.journalError })}</span>
        </p>
      )}
      <Section title={t('details.general')}>
        <div className="field-grid">
          <CommitField
            key={`name-${project.name}`}
            label={t('details.name')}
            value={project.name}
            testId="project-name"
            onCommit={(name) => {
              void setProperties({ name });
            }}
          />
          <CommitField
            key={`revision-${project.revision}`}
            label={t('details.revision')}
            value={project.revision}
            testId="project-revision"
            onCommit={(revision) => {
              void setProperties({ revision });
            }}
          />
        </div>
        <CommitField
          key={`description-${project.description}`}
          label={t('details.description')}
          value={project.description}
          multiline
          testId="project-description"
          onCommit={(description) => {
            void setProperties({ description });
          }}
        />
        <label className="field">
          <span className="field__label">{t('details.units')}</span>
          <select
            className="field__control"
            value={project.settings.units}
            data-testid="project-units"
            onChange={(event) => {
              const units = UNITS.find((unit) => unit === event.target.value);
              if (units !== undefined) {
                void setProperties({ settings: { units } });
              }
            }}
          >
            {UNITS.map((unit) => (
              <option key={unit} value={unit}>
                {t(`units.${unit}`)}
              </option>
            ))}
          </select>
        </label>
        <div className="field">
          <span className="field__label">{t('details.location')}</span>
          <div className="field__row">
            <code className="path" data-testid="project-path">
              {project.manifestPath}
            </code>
            <button
              type="button"
              className="button button--small"
              onClick={() => {
                showInFolder();
              }}
            >
              {t('project.showInFolder')}
            </button>
          </div>
        </div>
      </Section>
      <VariablesEditor project={project} />
      <Section title={t('details.statistics')}>
        <Stats
          items={[
            ['details.sheets', project.documents.filter((d) => d.kind === 'schematic').length],
            ['details.symbols', sum((d) => (d.kind === 'schematic' ? d.stats.symbols : 0))],
            ['details.footprints', board?.stats.footprints],
            ['details.nets', board?.stats.nets],
            ['details.tracks', board?.stats.tracks],
            ['details.vias', board?.stats.vias],
            ['details.copperLayers', board?.stats.copperLayers],
            [
              'details.boardSize',
              boardSize === undefined
                ? undefined
                : formatSize(boardSize[0], boardSize[1], units, language),
            ],
            ['details.schematicGrid', length(project.settings.schematicGrid)],
            ['details.boardGrid', length(project.settings.boardGrid)],
          ]}
        />
      </Section>
    </div>
  );
}

// --- Documents ----------------------------------------------------------------

const PAGE_FORMATS = ['a5', 'a4', 'a3', 'a2', 'a1', 'a0', 'letter', 'legal', 'tabloid'] as const;

function ResolvedPreview({
  value,
  doc,
}: {
  readonly value: string;
  readonly doc: string;
}): ReactNode {
  const { t } = useAppState();
  const { resolveText } = useProject();
  const [resolved, setResolved] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    if (!value.includes('${')) {
      return undefined;
    }
    void resolveText(value, doc).then((result) => {
      if (!cancelled) {
        setResolved(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [value, doc, resolveText]);
  if (!value.includes('${') || resolved === null) {
    return null;
  }
  return <span className="field__hint">{t('details.preview', { text: resolved })}</span>;
}

function SchematicDetails({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  const { setDocumentProperties } = useProject();
  const props = useDocumentProps(document);
  const page = record(props?.['page']);
  const titleBlock = record(props?.['titleBlock']);
  const { units, language } = useUnits();
  const size = document.stats.page;
  const fields = [
    ['title', 'details.tbTitle'],
    ['date', 'details.tbDate'],
    ['revision', 'details.tbRevision'],
    ['company', 'details.tbCompany'],
  ] as const;
  return (
    <div className="details__body" data-testid="details-schematic">
      <Section title={t('kinds.schematic')}>
        <DocumentHeader document={document} />
        <Stats
          items={[
            ['details.symbols', document.stats.symbols],
            ['details.wires', document.stats.wires],
            ['details.labels', document.stats.labels],
            ['details.sheets', document.stats.children?.length],
          ]}
        />
      </Section>
      {props !== null && (
        <Section title={t('details.page')}>
          <div className="field-grid">
            <label className="field">
              <span className="field__label">{t('details.format')}</span>
              <select
                className="field__control"
                value={text(page['format'])}
                data-testid="page-format"
                onChange={(event) => {
                  void setDocumentProperties(document.id, {
                    page: { format: event.target.value },
                  });
                }}
              >
                {PAGE_FORMATS.map((format) => (
                  <option key={format} value={format}>
                    {format.length === 2
                      ? format.toUpperCase()
                      : format.charAt(0).toUpperCase() + format.slice(1)}
                  </option>
                ))}
                <option value="custom">{t('details.customSize')}</option>
              </select>
            </label>
            <label className="field">
              <span className="field__label">{t('details.orientation')}</span>
              <select
                className="field__control"
                value={text(page['orientation']) === 'portrait' ? 'portrait' : 'landscape'}
                data-testid="page-orientation"
                onChange={(event) => {
                  void setDocumentProperties(document.id, {
                    page: { orientation: event.target.value },
                  });
                }}
              >
                <option value="landscape">{t('details.landscape')}</option>
                <option value="portrait">{t('details.portrait')}</option>
              </select>
            </label>
          </div>
          {size !== undefined && (
            <p className="field__hint">{formatSize(size[0], size[1], units, language)}</p>
          )}
        </Section>
      )}
      {props !== null && (
        <Section title={t('details.titleBlock')} testId="title-block">
          {fields.map(([key, label]) => (
            <div key={key} className="field--stack">
              <CommitField
                key={`${key}-${text(titleBlock[key])}`}
                label={t(label)}
                value={text(titleBlock[key])}
                testId={`title-${key}`}
                onCommit={(value) => {
                  void setDocumentProperties(document.id, { titleBlock: { [key]: value } });
                }}
              />
              <ResolvedPreview value={text(titleBlock[key])} doc={document.id} />
            </div>
          ))}
        </Section>
      )}
      <Coming text="details.comingSchematic" />
    </div>
  );
}

function stackupLabel(
  t: (key: MessageKey, vars?: Record<string, string | number>) => string,
  layer: Record<string, unknown>,
): string {
  const type = text(layer['type']);
  const side = text(layer['side']) === 'bottom' ? t('details.bottom') : t('details.top');
  switch (type) {
    case 'silkscreen':
      return `${t('details.silkscreen')} · ${side}`;
    case 'solderMask':
      return `${t('details.solderMask')} · ${side}`;
    case 'copper': {
      const name = text(layer['name']);
      if (name !== '') {
        return name;
      }
      const which = text(layer['layer']);
      if (which === 'cu.top') {
        return `${t('details.copper')} · ${t('details.top')}`;
      }
      if (which === 'cu.bottom') {
        return `${t('details.copper')} · ${t('details.bottom')}`;
      }
      return `${t('details.copper')} · ${t('details.inner', { n: which.replace('cu.in', '') })}`;
    }
    default:
      return text(layer['kind']) === 'prepreg' ? t('details.prepreg') : t('details.core');
  }
}

const COLORS: Readonly<Record<string, MessageKey>> = {
  white: 'colors.white',
  black: 'colors.black',
  green: 'colors.green',
  blue: 'colors.blue',
  red: 'colors.red',
  yellow: 'colors.yellow',
  purple: 'colors.purple',
};

/** Translates common colour names; other values are shown as entered. */
function colorName(t: (key: MessageKey) => string, value: string): string {
  const key = COLORS[value.toLowerCase()];
  return key === undefined ? value : t(key);
}

function PcbDetails({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  const props = useDocumentProps(document);
  const length = useLength();
  const { units, language } = useUnits();
  const stackup = record(props?.['stackup']);
  const layers = Array.isArray(stackup['layers']) ? stackup['layers'].map(record) : [];
  const size = document.stats.boardSize;
  return (
    <div className="details__body" data-testid="details-pcb">
      <Section title={t('kinds.pcb')}>
        <DocumentHeader document={document} />
        <Stats
          items={[
            [
              'details.boardSize',
              size === undefined
                ? t('details.noOutline')
                : formatSize(size[0], size[1], units, language),
            ],
            ['details.copperLayers', document.stats.copperLayers],
            [
              'details.thickness',
              document.stats.thickness === undefined ? undefined : length(document.stats.thickness),
            ],
            ['details.footprints', document.stats.footprints],
            ['details.tracks', document.stats.tracks],
            ['details.vias', document.stats.vias],
            ['details.zones', document.stats.zones],
            ['details.nets', document.stats.nets],
          ]}
        />
      </Section>
      {layers.length > 0 && (
        <Section title={t('details.stackup')} testId="stackup">
          <table className="table stackup">
            <thead>
              <tr>
                <th>{t('details.layer')}</th>
                <th>{t('details.material')}</th>
                <th className="table__number">{t('details.thickness')}</th>
              </tr>
            </thead>
            <tbody>
              {layers.map((layer, index) => (
                <tr
                  // The stack-up order is the identity of a layer.
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- stack-up rows have no id
                  key={index}
                  className={`stackup__row stackup__row--${text(layer['type']).replace(/[A-Z]/gu, (c) => `-${c.toLowerCase()}`)}`}
                >
                  <td>
                    <span className="stackup__swatch" aria-hidden="true" />
                    {stackupLabel(t, layer)}
                  </td>
                  <td>{text(layer['material']) || colorName(t, text(layer['color']))}</td>
                  <td className="table__number">
                    {num(layer['thickness']) > 0 ? length(num(layer['thickness'])) : ''}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}
      <Coming text="details.comingPcb" />
    </div>
  );
}

function LibraryDetails({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  return (
    <div className="details__body" data-testid="details-library">
      <Section title={t('kinds.library')}>
        <DocumentHeader document={document} />
        <Stats
          items={[
            ['details.symbols', document.stats.symbols],
            ['details.footprints', document.stats.footprints],
          ]}
        />
      </Section>
      <Coming text="details.comingLibrary" />
    </div>
  );
}

const CONSTRAINTS = [
  ['minClearance', 'details.minClearance'],
  ['minTrackWidth', 'details.minTrackWidth'],
  ['minViaDiameter', 'details.minViaDiameter'],
  ['minViaDrill', 'details.minViaDrill'],
  ['minAnnularRing', 'details.minAnnularRing'],
  ['minHole', 'details.minHole'],
  ['minHoleToHole', 'details.minHoleToHole'],
  ['minCopperToEdge', 'details.minCopperToEdge'],
  ['minSilkWidth', 'details.minSilkWidth'],
] as const;

function RulesDetails({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  const props = useDocumentProps(document);
  const length = useLength();
  const constraints = record(props?.['constraints']);
  const classes = Array.isArray(props?.['netClasses']) ? props['netClasses'].map(record) : [];
  return (
    <div className="details__body" data-testid="details-rules">
      <Section title={t('kinds.rules')}>
        <DocumentHeader document={document} />
      </Section>
      {props !== null && (
        <Section title={t('details.constraints')} testId="constraints">
          <dl className="stats stats--compact">
            {CONSTRAINTS.map(([key, label]) => (
              <div key={key} className="stats__item">
                <dt>{t(label)}</dt>
                <dd>{length(num(constraints[key]))}</dd>
              </div>
            ))}
          </dl>
        </Section>
      )}
      {classes.length > 0 && (
        <Section title={t('details.netClasses')} testId="net-classes">
          <table className="table">
            <thead>
              <tr>
                <th>{t('details.netClass')}</th>
                <th className="table__number">{t('details.clearance')}</th>
                <th className="table__number">{t('details.trackWidth')}</th>
                <th className="table__number">{t('details.viaDiameter')}</th>
                <th className="table__number">{t('details.viaDrill')}</th>
              </tr>
            </thead>
            <tbody>
              {classes.map((netClass) => (
                <tr key={text(netClass['name'])}>
                  <td>{text(netClass['name'])}</td>
                  <td className="table__number">{length(num(netClass['clearance']))}</td>
                  <td className="table__number">{length(num(netClass['trackWidth']))}</td>
                  <td className="table__number">{length(num(netClass['viaDiameter']))}</td>
                  <td className="table__number">{length(num(netClass['viaDrill']))}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Section>
      )}
      <Coming text="details.comingRules" />
    </div>
  );
}

const OUTPUT_KINDS: Readonly<Record<string, MessageKey>> = {
  gerber: 'outputKinds.gerber',
  drill: 'outputKinds.drill',
  drillMap: 'outputKinds.drillMap',
  bom: 'outputKinds.bom',
  pickAndPlace: 'outputKinds.pickAndPlace',
  pdf: 'outputKinds.pdf',
  svg: 'outputKinds.svg',
  step: 'outputKinds.step',
  ipc356: 'outputKinds.ipc356',
  ipc2581: 'outputKinds.ipc2581',
  netlist: 'outputKinds.netlist',
  archive: 'outputKinds.archive',
};

function OutputJobDetails({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t } = useAppState();
  const props = useDocumentProps(document);
  const outputs = Array.isArray(props?.['outputs']) ? props['outputs'].map(record) : [];
  return (
    <div className="details__body" data-testid="details-outputJob">
      <Section title={t('kinds.outputJob')}>
        <DocumentHeader document={document} />
        {props !== null && (
          <div className="field">
            <span className="field__label">{t('details.directory')}</span>
            <code className="path">{text(props['directory'])}</code>
          </div>
        )}
      </Section>
      {outputs.length > 0 && (
        <Section title={t('details.outputs')} testId="outputs">
          <table className="table">
            <thead>
              <tr>
                <th>{t('details.outputName')}</th>
                <th>{t('details.outputKind')}</th>
                <th>{t('details.enabled')}</th>
              </tr>
            </thead>
            <tbody>
              {outputs.map((output) => {
                const kind = OUTPUT_KINDS[text(output['kind'])];
                return (
                  <tr key={text(output['id'])}>
                    <td>{text(output['name'])}</td>
                    <td>{kind === undefined ? text(output['kind']) : t(kind)}</td>
                    <td>{output['enabled'] === false ? t('details.no') : t('details.yes')}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </Section>
      )}
    </div>
  );
}

function FileDetails({ file }: { readonly file: FileEntry }): ReactNode {
  const { t, snapshot } = useAppState();
  const { showInFolder } = useProject();
  const size = new Intl.NumberFormat(snapshot.language, {
    style: 'unit',
    unit: file.size >= 1_000_000 ? 'megabyte' : file.size >= 1_000 ? 'kilobyte' : 'byte',
    maximumFractionDigits: 1,
  }).format(
    file.size >= 1_000_000 ? file.size / 1e6 : file.size >= 1_000 ? file.size / 1e3 : file.size,
  );
  return (
    <div className="details__body" data-testid="details-file">
      <Section
        title={file.category === 'generated' ? t('details.generatedFile') : t('details.otherFile')}
      >
        <div className="field">
          <span className="field__label">{t('details.file')}</span>
          <div className="field__row">
            <code className="path">{file.path}</code>
            <button
              type="button"
              className="button button--small"
              onClick={() => {
                showInFolder(file.path);
              }}
            >
              {t('project.showInFolder')}
            </button>
          </div>
        </div>
        <Stats items={[['details.size', size]]} />
      </Section>
    </div>
  );
}

const GROUP_TITLE: Readonly<Record<GroupId, MessageKey>> = {
  schematics: 'tree.schematics',
  boards: 'tree.boards',
  libraries: 'tree.libraries',
  rules: 'tree.rules',
  outputJobs: 'tree.outputJobs',
  otherFiles: 'tree.otherFiles',
  generatedFiles: 'tree.generatedFiles',
};

function GroupDetails({ group }: { readonly group: GroupId }): ReactNode {
  const { t } = useAppState();
  const { project, files } = useProject();
  const kinds: Partial<Record<GroupId, DocumentSummary['kind']>> = {
    schematics: 'schematic',
    boards: 'pcb',
    libraries: 'library',
    rules: 'rules',
    outputJobs: 'outputJob',
  };
  const kind = kinds[group];
  const names =
    kind === undefined
      ? files
          .filter((file) => file.category === (group === 'generatedFiles' ? 'generated' : 'other'))
          .map((file) => file.path)
      : (project?.documents ?? []).filter((d) => d.kind === kind).map((d) => d.name);
  return (
    <div className="details__body" data-testid="details-group">
      <Section title={t(GROUP_TITLE[group])}>
        {names.length === 0 ? (
          <p className="field__hint">{t('tree.emptyGroup')}</p>
        ) : (
          <ul className="plain-list">
            {names.map((name) => (
              <li key={name}>{name}</li>
            ))}
          </ul>
        )}
      </Section>
    </div>
  );
}

/** The right-hand panel: details of the selected tree item. */
export function DocumentDetails({ target }: { readonly target: TreeTarget }): ReactNode {
  const { project } = useProject();
  if (project === null) {
    return null;
  }
  switch (target.type) {
    case 'project':
      return <ProjectOverview project={project} />;
    case 'group':
      return <GroupDetails group={target.group} />;
    case 'file':
      return <FileDetails file={target.file} />;
    case 'document': {
      // Always show the latest summary of the selected document.
      const document = project.documents.find((entry) => entry.id === target.document.id);
      if (document === undefined) {
        return <ProjectOverview project={project} />;
      }
      switch (document.kind) {
        case 'schematic':
          return <SchematicDetails key={document.id} document={document} />;
        case 'pcb':
          return <PcbDetails key={document.id} document={document} />;
        case 'library':
          return <LibraryDetails key={document.id} document={document} />;
        case 'rules':
          return <RulesDetails key={document.id} document={document} />;
        case 'outputJob':
          return <OutputJobDetails key={document.id} document={document} />;
      }
    }
  }
}
