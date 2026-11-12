/**
 * Input fields of the library editors. Every field keeps a draft while it
 * is edited and saves on Enter or when it loses focus; Escape restores the
 * stored value. A new stored value (after undo, for example) replaces the
 * draft.
 */
import { useState, type ReactNode } from 'react';

import { parseLength, toUnits, unitSymbol, type Units } from '@shared/project';

/** The draft of a field, reset when the stored value changes. */
function useDraft(value: string): [string, (next: string) => void] {
  const [draft, setDraft] = useState(value);
  const [stored, setStored] = useState(value);
  if (stored !== value) {
    setStored(value);
    setDraft(value);
  }
  return [draft, setDraft];
}

interface BaseProps {
  readonly label?: string;
  readonly testId?: string;
  readonly disabled?: boolean;
  /** No label element around the input (table cells). */
  readonly bare?: boolean;
  readonly ariaLabel?: string;
}

function Wrap({
  label,
  bare,
  children,
}: {
  readonly label: string | undefined;
  readonly bare: boolean | undefined;
  readonly children: ReactNode;
}): ReactNode {
  if (bare === true || label === undefined) {
    return children;
  }
  return (
    <label className="field field--compact">
      <span className="field__label">{label}</span>
      {children}
    </label>
  );
}

/** A text input. */
export function TextInput({
  label,
  value,
  testId,
  disabled,
  bare,
  ariaLabel,
  placeholder,
  multiline = false,
  onSave,
}: BaseProps & {
  readonly value: string;
  readonly placeholder?: string;
  readonly multiline?: boolean;
  readonly onSave: (value: string) => void;
}): ReactNode {
  const [draft, setDraft] = useDraft(value);
  const save = (): void => {
    if (draft !== value) {
      onSave(draft);
    }
  };
  const common = {
    className: 'field__control',
    value: draft,
    disabled,
    placeholder,
    spellCheck: false,
    'aria-label': ariaLabel ?? (bare === true ? label : undefined),
    'data-testid': testId,
    onBlur: save,
  };
  return (
    <Wrap label={label} bare={bare}>
      {multiline ? (
        <textarea
          {...common}
          rows={2}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Escape') {
              setDraft(value);
            }
          }}
        />
      ) : (
        <input
          {...common}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              save();
            } else if (event.key === 'Escape') {
              setDraft(value);
            }
          }}
        />
      )}
    </Wrap>
  );
}

/** A length in the project's units; saves nanometres. */
export function LengthInput({
  label,
  value,
  units,
  testId,
  disabled,
  bare,
  ariaLabel,
  min,
  onSave,
}: BaseProps & {
  readonly value: number;
  readonly units: Units;
  /** Smallest accepted value (nm). */
  readonly min?: number;
  readonly onSave: (value: number) => void;
}): ReactNode {
  const shown = toUnits(value, units);
  const [draft, setDraft] = useDraft(shown);
  const save = (): void => {
    const parsed = parseLength(draft, units);
    if (parsed === null || (min !== undefined && parsed < min)) {
      setDraft(shown);
      return;
    }
    if (parsed !== value) {
      onSave(parsed);
    }
  };
  return (
    <Wrap label={label} bare={bare}>
      <span className="length-input">
        <input
          className="field__control length-input__control"
          value={draft}
          disabled={disabled}
          inputMode="decimal"
          aria-label={ariaLabel ?? (bare === true ? label : undefined)}
          data-testid={testId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              save();
            } else if (event.key === 'Escape') {
              setDraft(shown);
            }
          }}
        />
        <span className="length-input__unit">{unitSymbol(units)}</span>
      </span>
    </Wrap>
  );
}

/** A plain number (angles, scale factors, counts). */
export function NumberInput({
  label,
  value,
  testId,
  disabled,
  bare,
  ariaLabel,
  integer = false,
  min,
  max,
  suffix,
  onSave,
}: BaseProps & {
  readonly value: number;
  readonly integer?: boolean;
  readonly min?: number;
  readonly max?: number;
  readonly suffix?: string;
  readonly onSave: (value: number) => void;
}): ReactNode {
  const shown = String(value);
  const [draft, setDraft] = useDraft(shown);
  const save = (): void => {
    const parsed = Number(draft.trim().replace(',', '.'));
    if (
      draft.trim() === '' ||
      !Number.isFinite(parsed) ||
      (integer && !Number.isInteger(parsed)) ||
      (min !== undefined && parsed < min) ||
      (max !== undefined && parsed > max)
    ) {
      setDraft(shown);
      return;
    }
    if (parsed !== value) {
      onSave(parsed);
    }
  };
  return (
    <Wrap label={label} bare={bare}>
      <span className="length-input">
        <input
          className="field__control length-input__control"
          value={draft}
          disabled={disabled}
          inputMode={integer ? 'numeric' : 'decimal'}
          aria-label={ariaLabel ?? (bare === true ? label : undefined)}
          data-testid={testId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              save();
            } else if (event.key === 'Escape') {
              setDraft(shown);
            }
          }}
        />
        {suffix !== undefined && <span className="length-input__unit">{suffix}</span>}
      </span>
    </Wrap>
  );
}

/** Two or three lengths side by side (a point, a size). */
export function LengthTuple({
  label,
  values,
  units,
  testId,
  min,
  onSave,
}: {
  readonly label: string;
  readonly values: readonly number[];
  readonly units: Units;
  readonly testId?: string;
  readonly min?: number;
  readonly onSave: (values: number[]) => void;
}): ReactNode {
  return (
    <fieldset className="field field--compact field--tuple">
      <legend className="field__label">{label}</legend>
      <span className="field__row">
        {values.map((value, index) => (
          <LengthInput
            // The positions of a tuple never move.
            // eslint-disable-next-line @eslint-react/no-array-index-key -- fixed positions
            key={index}
            bare
            ariaLabel={`${label} ${String(index + 1)}`}
            value={value}
            units={units}
            {...(min === undefined ? {} : { min })}
            {...(testId === undefined ? {} : { testId: `${testId}-${String(index)}` })}
            onSave={(next) => {
              const copy = [...values];
              copy[index] = next;
              onSave(copy);
            }}
          />
        ))}
      </span>
    </fieldset>
  );
}

/** A select with translated options. */
export function SelectInput<T extends string>({
  label,
  value,
  options,
  testId,
  disabled,
  bare,
  ariaLabel,
  onSave,
}: BaseProps & {
  readonly value: T | '';
  readonly options: readonly (readonly [T | '', string])[];
  readonly onSave: (value: T) => void;
}): ReactNode {
  return (
    <Wrap label={label} bare={bare}>
      <select
        className="field__control"
        value={value}
        disabled={disabled}
        aria-label={ariaLabel ?? (bare === true ? label : undefined)}
        data-testid={testId}
        onChange={(event) => {
          onSave(event.target.value as T);
        }}
      >
        {options.map(([key, text]) => (
          <option key={key} value={key}>
            {text}
          </option>
        ))}
      </select>
    </Wrap>
  );
}

/** A checkbox. */
export function CheckInput({
  label,
  checked,
  testId,
  disabled,
  onSave,
}: {
  readonly label: string;
  readonly checked: boolean;
  readonly testId?: string;
  readonly disabled?: boolean;
  readonly onSave: (checked: boolean) => void;
}): ReactNode {
  return (
    <label className="checkbox">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        data-testid={testId}
        onChange={(event) => {
          onSave(event.target.checked);
        }}
      />
      <span>{label}</span>
    </label>
  );
}
