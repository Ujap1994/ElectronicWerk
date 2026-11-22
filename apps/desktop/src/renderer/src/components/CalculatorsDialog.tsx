/**
 * Engineering calculators: voltage divider, regulator feedback resistors,
 * E series, current of tracks and vias (IPC-2152, IPC-2221) and fusing
 * current (Onderdonk). They compute while you type; the engine does the
 * maths (`calc.*`), so the command line and AI agents get the same values.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { formatLength, parseLength, unitSymbol, type Units } from '@shared/project';
import { formatQuantity, parseQuantity } from '@shared/quantity';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { Modal } from './Modal';

type CalcId = 'divider' | 'regulator' | 'series' | 'track' | 'via' | 'fusing';
const CALCS: readonly CalcId[] = ['divider', 'regulator', 'series', 'track', 'via', 'fusing'];
const SERIES = ['E3', 'E6', 'E12', 'E24', 'E48', 'E96', 'E192'] as const;
type Series = (typeof SERIES)[number];
/** Copper weights in ounces per square foot and their thickness (nm). */
const COPPER: readonly (readonly [string, number])[] = [
  ['0.5', 17_500],
  ['1', 35_000],
  ['2', 70_000],
  ['3', 105_000],
];
const STORED = 'ew.calculators.active';

/** Runs a calculator whenever its parameters change (`null`: invalid input). */
// eslint-disable-next-line @typescript-eslint/no-unnecessary-type-parameters -- the reply type of the method
function useCalc<T>(
  method: string,
  params: object | null,
): { result: T | null; error: string | null } {
  const { engine } = useAppState();
  const key = params === null ? null : JSON.stringify(params);
  const [state, setState] = useState<{
    key: string | null;
    result: T | null;
    error: string | null;
  }>({ key: null, result: null, error: null });
  useEffect(() => {
    if (key === null) {
      return undefined;
    }
    let cancelled = false;
    engine.call<T>(method, JSON.parse(key) as unknown).then(
      (result) => {
        if (!cancelled) {
          setState({ key, result, error: null });
        }
      },
      (error: unknown) => {
        if (!cancelled) {
          setState({
            key,
            result: null,
            error: error instanceof Error ? error.message : String(error),
          });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [engine, method, key]);
  return key === null || state.key !== key ? { result: null, error: null } : state;
}

function Field({
  label,
  value,
  onChange,
  valid,
  suffix,
  testId,
}: {
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly valid: boolean;
  readonly suffix?: string;
  readonly testId: string;
}): ReactNode {
  return (
    <label className="field calculators__field">
      <span className="field__label">{label}</span>
      <span className="calculators__input">
        <input
          className="field__control"
          value={value}
          aria-invalid={!valid}
          data-testid={testId}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
        {suffix !== undefined && <span className="calculators__suffix">{suffix}</span>}
      </span>
    </label>
  );
}

function Choice<T extends string>({
  label,
  value,
  options,
  onChange,
  testId,
}: {
  readonly label: string;
  readonly value: T;
  readonly options: readonly (readonly [T, string])[];
  readonly onChange: (value: T) => void;
  readonly testId: string;
}): ReactNode {
  return (
    <label className="field calculators__field">
      <span className="field__label">{label}</span>
      <select
        className="field__control"
        value={value}
        data-testid={testId}
        onChange={(event) => {
          const found = options.find(([v]) => v === event.target.value);
          if (found !== undefined) {
            onChange(found[0]);
          }
        }}
      >
        {options.map(([v, text]) => (
          <option key={v} value={v}>
            {text}
          </option>
        ))}
      </select>
    </label>
  );
}

function Rows({
  rows,
}: {
  readonly rows: readonly (readonly [string, string, string])[];
}): ReactNode {
  return (
    <dl className="calculators__result" data-testid="calc-result">
      {rows.map(([key, label, value]) => (
        <div key={key} className="calculators__row">
          <dt>{label}</dt>
          <dd data-testid={`calc-${key}`}>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

function Problem({ error }: { readonly error: string | null }): ReactNode {
  const { t } = useAppState();
  return (
    <p className="calculators__problem" role="status">
      {error ?? t('calc.invalid')}
    </p>
  );
}

/** Two resistors between a voltage and ground, the output between them. */
function DividerPicture({
  input,
  upper,
  lower,
  out,
}: {
  readonly input: string;
  readonly upper: string;
  readonly lower: string;
  readonly out: string;
}): ReactNode {
  return (
    <svg className="calculators__picture" viewBox="0 0 120 150" aria-hidden="true">
      <path d="M40 10v20M40 70v20M40 130v10M30 140h20M34 144h12" />
      <rect x="32" y="30" width="16" height="40" rx="2" />
      <rect x="32" y="90" width="16" height="40" rx="2" />
      <path d="M40 80h40" />
      <circle cx="40" cy="80" r="2.5" className="calculators__dot" />
      <circle cx="40" cy="10" r="2.5" className="calculators__dot" />
      <text x="48" y="14">
        {input}
      </text>
      <text x="56" y="54">
        {upper}
      </text>
      <text x="56" y="114">
        {lower}
      </text>
      <text x="84" y="84">
        {out}
      </text>
    </svg>
  );
}

const ok = (value: number | null): value is number => value !== null && value > 0;

function useSeries(): [Series, (s: Series) => void] {
  return useState<Series>('E24');
}

function DividerCalc(): ReactNode {
  const { t, snapshot } = useAppState();
  const locale = snapshot.language;
  const [vin, setVin] = useState('12');
  const [vout, setVout] = useState('3.3');
  const [series, setSeries] = useSeries();
  const [min, setMin] = useState('1k');
  const [max, setMax] = useState('1M');
  const values = {
    vin: parseQuantity(vin),
    vout: parseQuantity(vout),
    minTotal: parseQuantity(min),
    maxTotal: parseQuantity(max),
  };
  const valid = ok(values.vin) && ok(values.vout) && ok(values.minTotal) && ok(values.maxTotal);
  const { result, error } = useCalc<{
    pairs: { r1: number; r2: number; vout: number; error: number; current: number }[];
  }>('calc.divider', valid ? { ...values, series } : null);
  return (
    <>
      <p className="field__hint">{t('calc.divider.hint')}</p>
      <div className="calculators__inputs">
        <DividerPicture input="Vin" upper="R1" lower="R2" out="Vout" />
        <div className="calculators__fields">
          <Field
            label={t('calc.divider.vin')}
            value={vin}
            onChange={setVin}
            valid={ok(values.vin)}
            suffix="V"
            testId="calc-vin"
          />
          <Field
            label={t('calc.divider.vout')}
            value={vout}
            onChange={setVout}
            valid={ok(values.vout)}
            suffix="V"
            testId="calc-vout"
          />
          <Choice
            label={t('calc.series')}
            value={series}
            options={SERIES.map((s) => [s, s] as const)}
            onChange={setSeries}
            testId="calc-series"
          />
          <Field
            label={t('calc.divider.minTotal')}
            value={min}
            onChange={setMin}
            valid={ok(values.minTotal)}
            suffix="Ω"
            testId="calc-min"
          />
          <Field
            label={t('calc.divider.maxTotal')}
            value={max}
            onChange={setMax}
            valid={ok(values.maxTotal)}
            suffix="Ω"
            testId="calc-max"
          />
        </div>
      </div>
      {result === null ? (
        (!valid || error !== null) && <Problem error={error} />
      ) : (
        <PairTable
          heads={['R1', 'R2', t('calc.divider.output'), t('calc.error'), t('calc.current')]}
          rows={result.pairs.map((p) => [
            formatQuantity(p.r1, 'Ω', locale),
            formatQuantity(p.r2, 'Ω', locale),
            formatQuantity(p.vout, 'V', locale),
            percent(p.error, locale),
            formatQuantity(p.current, 'A', locale, 3),
          ])}
        />
      )}
    </>
  );
}

function percent(value: number, locale: string): string {
  const text = new Intl.NumberFormat(locale, {
    maximumFractionDigits: 3,
    signDisplay: 'exceptZero',
  }).format(value);
  return `${text} %`;
}

function PairTable({
  heads,
  rows,
}: {
  readonly heads: readonly string[];
  readonly rows: readonly (readonly string[])[];
}): ReactNode {
  const { t } = useAppState();
  if (rows.length === 0) {
    return <p className="calculators__problem">{t('calc.noPairs')}</p>;
  }
  return (
    <table className="table calculators__table" data-testid="calc-pairs">
      <thead>
        <tr>
          {heads.map((head) => (
            <th key={head}>{head}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row, index) => (
          // eslint-disable-next-line @eslint-react/no-array-index-key -- rows have no identity
          <tr key={index} data-testid="calc-pair">
            {row.map((cell, column) => (
              // eslint-disable-next-line @eslint-react/no-array-index-key -- fixed columns
              <td key={column}>{cell}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function RegulatorCalc(): ReactNode {
  const { t, snapshot } = useAppState();
  const locale = snapshot.language;
  const [vref, setVref] = useState('1.25');
  const [vout, setVout] = useState('5');
  const [iadj, setIadj] = useState('0');
  const [series, setSeries] = useSeries();
  const [min, setMin] = useState('100');
  const [max, setMax] = useState('1M');
  const values = {
    vref: parseQuantity(vref),
    vout: parseQuantity(vout),
    iadj: parseQuantity(iadj),
    minLower: parseQuantity(min),
    maxLower: parseQuantity(max),
  };
  const iadjOk = values.iadj !== null && values.iadj >= 0;
  const valid =
    ok(values.vref) && ok(values.vout) && iadjOk && ok(values.minLower) && ok(values.maxLower);
  const { result, error } = useCalc<{
    pairs: { upper: number; lower: number; vout: number; error: number; current: number }[];
  }>('calc.regulator', valid ? { ...values, series } : null);
  return (
    <>
      <p className="field__hint">{t('calc.regulator.hint')}</p>
      <div className="calculators__inputs">
        <DividerPicture input="Vout" upper="R1" lower="R2" out="FB / ADJ" />
        <div className="calculators__fields">
          <Field
            label={t('calc.regulator.vref')}
            value={vref}
            onChange={setVref}
            valid={ok(values.vref)}
            suffix="V"
            testId="calc-vref"
          />
          <Field
            label={t('calc.regulator.vout')}
            value={vout}
            onChange={setVout}
            valid={ok(values.vout)}
            suffix="V"
            testId="calc-vout"
          />
          <Field
            label={t('calc.regulator.iadj')}
            value={iadj}
            onChange={setIadj}
            valid={iadjOk}
            suffix="A"
            testId="calc-iadj"
          />
          <Choice
            label={t('calc.series')}
            value={series}
            options={SERIES.map((s) => [s, s] as const)}
            onChange={setSeries}
            testId="calc-series"
          />
          <Field
            label={t('calc.regulator.minLower')}
            value={min}
            onChange={setMin}
            valid={ok(values.minLower)}
            suffix="Ω"
            testId="calc-min"
          />
          <Field
            label={t('calc.regulator.maxLower')}
            value={max}
            onChange={setMax}
            valid={ok(values.maxLower)}
            suffix="Ω"
            testId="calc-max"
          />
        </div>
      </div>
      {result === null ? (
        (!valid || error !== null) && <Problem error={error} />
      ) : (
        <PairTable
          heads={['R1', 'R2', t('calc.regulator.output'), t('calc.error'), t('calc.current')]}
          rows={result.pairs.map((p) => [
            formatQuantity(p.upper, 'Ω', locale),
            formatQuantity(p.lower, 'Ω', locale),
            formatQuantity(p.vout, 'V', locale),
            percent(p.error, locale),
            formatQuantity(p.current, 'A', locale, 3),
          ])}
        />
      )}
    </>
  );
}

interface SeriesResult {
  nearest: number;
  below: number;
  above: number;
  error: number;
  tolerance: number;
}

function SeriesCalc(): ReactNode {
  const { t, snapshot, engine } = useAppState();
  const locale = snapshot.language;
  const [text, setText] = useState('4.6k');
  const value = parseQuantity(text);
  const [rows, setRows] = useState<{ key: string; list: SeriesResult[] } | null>(null);
  const key = ok(value) ? String(value) : null;
  useEffect(() => {
    if (key === null) {
      return undefined;
    }
    let cancelled = false;
    void Promise.all(
      SERIES.map((series) =>
        engine.call<SeriesResult>('calc.eSeries', { value: Number(key), series }),
      ),
    ).then(
      (list) => {
        if (!cancelled) {
          setRows({ key, list });
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, key]);
  return (
    <>
      <p className="field__hint">{t('calc.eSeries.hint')}</p>
      <div className="calculators__fields calculators__fields--one">
        <Field
          label={t('calc.eSeries.value')}
          value={text}
          onChange={setText}
          valid={ok(value)}
          testId="calc-value"
        />
      </div>
      {key === null ? (
        <Problem error={null} />
      ) : (
        rows?.key === key && (
          <PairTable
            heads={[
              t('calc.series'),
              t('calc.eSeries.nearest'),
              t('calc.eSeries.below'),
              t('calc.eSeries.above'),
              t('calc.error'),
            ]}
            rows={rows.list.map((r, i) => [
              `${SERIES[i] ?? ''} (±${String(r.tolerance)} %)`,
              formatQuantity(r.nearest, '', locale).trim(),
              formatQuantity(r.below, '', locale).trim(),
              formatQuantity(r.above, '', locale).trim(),
              percent(r.error, locale),
            ])}
          />
        )
      )}
    </>
  );
}

interface CurrentResult {
  current: number;
  width?: number;
  area: number;
  resistance: number;
  voltageDrop: number;
  power: number;
  temperature?: number;
  notes: string[];
}

/** A cross-section in the units of the project. */
function area(mm2: number, units: Units, locale: string): string {
  const mils = units !== 'mm';
  const value = mils ? mm2 / (0.0254 * 0.0254) : mm2;
  const text = new Intl.NumberFormat(locale, { maximumSignificantDigits: 4 }).format(value);
  return `${text} ${mils ? 'mil²' : 'mm²'}`;
}

function Notes({ notes }: { readonly notes: readonly string[] }): ReactNode {
  const { t } = useAppState();
  return (
    <>
      {notes.map((note) => (
        <p key={note} className="calculators__note" data-testid={`calc-note-${note}`}>
          {t(`calc.notes.${note}` as MessageKey)}
        </p>
      ))}
    </>
  );
}

function useThermal(): {
  rise: string;
  setRise: (v: string) => void;
  ambient: string;
  setAmbient: (v: string) => void;
  standard: 'ipc2152' | 'ipc2221';
  setStandard: (v: 'ipc2152' | 'ipc2221') => void;
  values: { rise: number | null; ambient: number | null };
} {
  const [rise, setRise] = useState('10');
  const [ambient, setAmbient] = useState('25');
  const [standard, setStandard] = useState<'ipc2152' | 'ipc2221'>('ipc2152');
  return {
    rise,
    setRise,
    ambient,
    setAmbient,
    standard,
    setStandard,
    values: { rise: parseQuantity(rise, ['°C', 'K']), ambient: parseQuantity(ambient, ['°C']) },
  };
}

function ThermalFields({
  thermal,
}: {
  readonly thermal: ReturnType<typeof useThermal>;
}): ReactNode {
  const { t } = useAppState();
  return (
    <>
      <Field
        label={t('calc.rise')}
        value={thermal.rise}
        onChange={thermal.setRise}
        valid={ok(thermal.values.rise)}
        suffix="°C"
        testId="calc-rise"
      />
      <Field
        label={t('calc.ambient')}
        value={thermal.ambient}
        onChange={thermal.setAmbient}
        valid={thermal.values.ambient !== null}
        suffix="°C"
        testId="calc-ambient"
      />
      <Choice
        label={t('calc.standard')}
        value={thermal.standard}
        options={[
          ['ipc2152', t('calc.standards.ipc2152')],
          ['ipc2221', t('calc.standards.ipc2221')],
        ]}
        onChange={thermal.setStandard}
        testId="calc-standard"
      />
    </>
  );
}

function TrackCalc({ units }: { readonly units: Units }): ReactNode {
  const { t, snapshot } = useAppState();
  const locale = snapshot.language;
  const [mode, setMode] = useState<'width' | 'current'>('width');
  const [current, setCurrent] = useState('1');
  const [width, setWidth] = useState(units === 'mm' ? '0.5' : '20');
  const [copper, setCopper] = useState('1');
  const [length, setLength] = useState(units === 'mm' ? '10' : '400');
  const [placement, setPlacement] = useState<'external' | 'internal'>('external');
  const thermal = useThermal();
  const values = {
    current: parseQuantity(current),
    width: parseLength(width, units),
    length: parseLength(length, units),
  };
  const thickness = COPPER.find(([oz]) => oz === copper)?.[1] ?? 35_000;
  const given = mode === 'width' ? ok(values.current) : ok(values.width);
  const valid =
    given && ok(values.length) && ok(thermal.values.rise) && thermal.values.ambient !== null;
  const { result, error } = useCalc<CurrentResult>(
    'calc.track',
    valid
      ? {
          ...(mode === 'width' ? { current: values.current } : { width: values.width }),
          thickness,
          length: values.length,
          rise: thermal.values.rise,
          ambient: thermal.values.ambient,
          standard: thermal.standard,
          placement,
        }
      : null,
  );
  return (
    <>
      <p className="field__hint">{t('calc.track.hint')}</p>
      <div className="segmented" role="tablist">
        {(['width', 'current'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={mode === value}
            className={
              mode === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`calc-mode-${value}`}
            onClick={() => {
              setMode(value);
            }}
          >
            {t(`calc.track.modes.${value}`)}
          </button>
        ))}
      </div>
      <div className="calculators__fields">
        {mode === 'width' ? (
          <Field
            label={t('calc.current')}
            value={current}
            onChange={setCurrent}
            valid={ok(values.current)}
            suffix="A"
            testId="calc-current-in"
          />
        ) : (
          <Field
            label={t('calc.track.width')}
            value={width}
            onChange={setWidth}
            valid={ok(values.width)}
            suffix={unitSymbol(units)}
            testId="calc-width-in"
          />
        )}
        <Choice
          label={t('calc.copper')}
          value={copper}
          options={COPPER.map(([oz, nm]) => [oz, `${oz} oz (${String(nm / 1000)} µm)`] as const)}
          onChange={setCopper}
          testId="calc-copper"
        />
        <Field
          label={t('calc.track.length')}
          value={length}
          onChange={setLength}
          valid={ok(values.length)}
          suffix={unitSymbol(units)}
          testId="calc-length"
        />
        <ThermalFields thermal={thermal} />
        {thermal.standard === 'ipc2221' && (
          <Choice
            label={t('calc.placement')}
            value={placement}
            options={[
              ['external', t('calc.placements.external')],
              ['internal', t('calc.placements.internal')],
            ]}
            onChange={setPlacement}
            testId="calc-placement"
          />
        )}
      </div>
      {result === null ? (
        (!valid || error !== null) && <Problem error={error} />
      ) : (
        <>
          <Rows
            rows={[
              mode === 'width'
                ? [
                    'width',
                    t('calc.track.width'),
                    formatLength(Math.round(result.width ?? 0), units, locale),
                  ]
                : ['current', t('calc.current'), formatQuantity(result.current, 'A', locale, 3)],
              ['area', t('calc.area'), area(result.area, units, locale)],
              [
                'resistance',
                t('calc.resistance'),
                formatQuantity(result.resistance, 'Ω', locale, 3),
              ],
              ['drop', t('calc.voltageDrop'), formatQuantity(result.voltageDrop, 'V', locale, 3)],
              ['power', t('calc.power'), formatQuantity(result.power, 'W', locale, 3)],
            ]}
          />
          <Notes notes={result.notes} />
        </>
      )}
      <p className="field__hint">{t(`calc.standards.${thermal.standard}Note`)}</p>
    </>
  );
}

function ViaCalc({ units }: { readonly units: Units }): ReactNode {
  const { t, snapshot } = useAppState();
  const locale = snapshot.language;
  const [drill, setDrill] = useState(units === 'mm' ? '0.3' : '12');
  const [plating, setPlating] = useState('25');
  const [board, setBoard] = useState(units === 'mm' ? '1.6' : '63');
  const thermal = useThermal();
  const values = {
    drill: parseLength(drill, units),
    plating: parseQuantity(plating),
    length: parseLength(board, units),
  };
  const valid =
    ok(values.drill) &&
    ok(values.plating) &&
    ok(values.length) &&
    ok(thermal.values.rise) &&
    thermal.values.ambient !== null;
  const { result, error } = useCalc<CurrentResult>(
    'calc.via',
    valid
      ? {
          drill: values.drill,
          plating: (values.plating ?? 0) * 1000,
          length: values.length,
          rise: thermal.values.rise,
          ambient: thermal.values.ambient,
          standard: thermal.standard,
        }
      : null,
  );
  return (
    <>
      <p className="field__hint">{t('calc.via.hint')}</p>
      <div className="calculators__fields">
        <Field
          label={t('calc.via.drill')}
          value={drill}
          onChange={setDrill}
          valid={ok(values.drill)}
          suffix={unitSymbol(units)}
          testId="calc-drill"
        />
        <Field
          label={t('calc.via.plating')}
          value={plating}
          onChange={setPlating}
          valid={ok(values.plating)}
          suffix="µm"
          testId="calc-plating"
        />
        <Field
          label={t('calc.via.board')}
          value={board}
          onChange={setBoard}
          valid={ok(values.length)}
          suffix={unitSymbol(units)}
          testId="calc-board"
        />
        <ThermalFields thermal={thermal} />
      </div>
      {result === null ? (
        (!valid || error !== null) && <Problem error={error} />
      ) : (
        <>
          <Rows
            rows={[
              ['current', t('calc.current'), formatQuantity(result.current, 'A', locale, 3)],
              ['area', t('calc.area'), area(result.area, units, locale)],
              [
                'resistance',
                t('calc.resistance'),
                formatQuantity(result.resistance, 'Ω', locale, 3),
              ],
              ['drop', t('calc.voltageDrop'), formatQuantity(result.voltageDrop, 'V', locale, 3)],
            ]}
          />
          <Notes notes={result.notes} />
        </>
      )}
    </>
  );
}

function FusingCalc({ units }: { readonly units: Units }): ReactNode {
  const { t, snapshot } = useAppState();
  const locale = snapshot.language;
  const [kind, setKind] = useState<'track' | 'via'>('track');
  const [width, setWidth] = useState(units === 'mm' ? '0.25' : '10');
  const [copper, setCopper] = useState('1');
  const [drill, setDrill] = useState(units === 'mm' ? '0.3' : '12');
  const [plating, setPlating] = useState('25');
  const [time, setTime] = useState('1');
  const [ambient, setAmbient] = useState('25');
  const values = {
    width: parseLength(width, units),
    drill: parseLength(drill, units),
    plating: parseQuantity(plating),
    time: parseQuantity(time),
    ambient: parseQuantity(ambient, ['°C']),
  };
  const thickness = COPPER.find(([oz]) => oz === copper)?.[1] ?? 35_000;
  const shape = kind === 'track' ? ok(values.width) : ok(values.drill) && ok(values.plating);
  const valid = shape && ok(values.time) && values.ambient !== null;
  const { result, error } = useCalc<{ current: number; area: number; notes: string[] }>(
    'calc.fusing',
    valid
      ? {
          ...(kind === 'track'
            ? { width: values.width, thickness }
            : { drill: values.drill, plating: (values.plating ?? 0) * 1000 }),
          time: values.time,
          ambient: values.ambient,
        }
      : null,
  );
  return (
    <>
      <p className="field__hint">{t('calc.fusing.hint')}</p>
      <div className="segmented" role="tablist">
        {(['track', 'via'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={kind === value}
            className={
              kind === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`calc-kind-${value}`}
            onClick={() => {
              setKind(value);
            }}
          >
            {t(`calc.fusing.kinds.${value}`)}
          </button>
        ))}
      </div>
      <div className="calculators__fields">
        {kind === 'track' ? (
          <>
            <Field
              label={t('calc.track.width')}
              value={width}
              onChange={setWidth}
              valid={ok(values.width)}
              suffix={unitSymbol(units)}
              testId="calc-width-in"
            />
            <Choice
              label={t('calc.copper')}
              value={copper}
              options={COPPER.map(
                ([oz, nm]) => [oz, `${oz} oz (${String(nm / 1000)} µm)`] as const,
              )}
              onChange={setCopper}
              testId="calc-copper"
            />
          </>
        ) : (
          <>
            <Field
              label={t('calc.via.drill')}
              value={drill}
              onChange={setDrill}
              valid={ok(values.drill)}
              suffix={unitSymbol(units)}
              testId="calc-drill"
            />
            <Field
              label={t('calc.via.plating')}
              value={plating}
              onChange={setPlating}
              valid={ok(values.plating)}
              suffix="µm"
              testId="calc-plating"
            />
          </>
        )}
        <Field
          label={t('calc.fusing.time')}
          value={time}
          onChange={setTime}
          valid={ok(values.time)}
          suffix="s"
          testId="calc-time"
        />
        <Field
          label={t('calc.ambient')}
          value={ambient}
          onChange={setAmbient}
          valid={values.ambient !== null}
          suffix="°C"
          testId="calc-ambient"
        />
      </div>
      {result === null ? (
        (!valid || error !== null) && <Problem error={error} />
      ) : (
        <>
          <Rows
            rows={[
              ['current', t('calc.fusing.current'), formatQuantity(result.current, 'A', locale, 3)],
              ['area', t('calc.area'), area(result.area, units, locale)],
            ]}
          />
          <Notes notes={result.notes} />
        </>
      )}
      <p className="field__hint">{t('calc.fusing.note')}</p>
    </>
  );
}

function readStored(): CalcId | null {
  try {
    const value = window.localStorage.getItem(STORED);
    return CALCS.find((c) => c === value) ?? null;
  } catch {
    return null;
  }
}

export function CalculatorsDialog({ onClose }: { readonly onClose: () => void }): ReactNode {
  const { t } = useAppState();
  const { project } = useProject();
  const units: Units = project?.settings.units ?? 'mm';
  const [active, setActive] = useState<CalcId>(() => readStored() ?? 'divider');
  const choose = (id: CalcId): void => {
    setActive(id);
    try {
      window.localStorage.setItem(STORED, id);
    } catch {
      // The choice is only a convenience.
    }
  };
  return (
    <Modal title={t('calc.title')} onClose={onClose} testId="calculators" className="calculators">
      <div className="calculators__body">
        <nav className="calculators__nav" role="tablist" aria-orientation="vertical">
          {CALCS.map((id) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={active === id}
              className="calculators__tab"
              data-testid={`calc-tab-${id}`}
              onClick={() => {
                choose(id);
              }}
            >
              {t(`calc.tabs.${id}`)}
            </button>
          ))}
        </nav>
        <section
          className="calculators__panel"
          role="tabpanel"
          aria-label={t(`calc.tabs.${active}`)}
        >
          {active === 'divider' && <DividerCalc />}
          {active === 'regulator' && <RegulatorCalc />}
          {active === 'series' && <SeriesCalc />}
          {active === 'track' && <TrackCalc units={units} />}
          {active === 'via' && <ViaCalc units={units} />}
          {active === 'fusing' && <FusingCalc units={units} />}
        </section>
      </div>
    </Modal>
  );
}
