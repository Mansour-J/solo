// The vehicle form: values, validation (mirroring the API) and conversion to the API shape.
import { z } from 'zod';
import type { FuelType, MileageRate, Vehicle, VehicleInput, VehicleType } from './api';
import { defaultRateName, emptyNewRateValues, newRateSchema, toNewRateInput, type NewRateValues } from './rateForm';

export interface VehicleFormValues {
  name: string;
  registration: string;
  make: string;
  model: string;
  /** NumberInput reports '' when cleared. */
  year: number | string;
  vehicle_type: VehicleType;
  fuel_type: FuelType;
  default_rate_id: string | null;
  /** Create `new_rate` with the vehicle and make it the default, instead of `default_rate_id`. */
  create_rate: boolean;
  new_rate: NewRateValues;
  is_default: boolean;
  notes: string;
}

/** The default-rate option that creates a rate in the vehicle form (rate ids are UUIDs). */
export const NEW_RATE_OPTION = 'new-rate';

/**
 * A new vehicle. It starts with the only active rate, or in "new rate" mode when there are no active
 * rates; the new rate's first day is `newRateFrom` (the start of the financial year).
 */
export function emptyVehicleValues({
  rates,
  newRateFrom,
  firstVehicle,
}: {
  rates: MileageRate[];
  newRateFrom: string | null;
  firstVehicle: boolean;
}): VehicleFormValues {
  const active = rates.filter((r) => !r.archived);
  return {
    name: '',
    registration: '',
    make: '',
    model: '',
    year: '',
    vehicle_type: 'car',
    fuel_type: 'petrol',
    default_rate_id: active.length === 1 ? active[0]!.id : null,
    create_rate: active.length === 0,
    new_rate: emptyNewRateValues(newRateFrom, defaultRateName(rates)),
    is_default: firstVehicle,
    notes: '',
  };
}

export function fromVehicle(vehicle: Vehicle, newRate: NewRateValues = emptyNewRateValues(null)): VehicleFormValues {
  return {
    name: vehicle.name,
    registration: vehicle.registration,
    make: vehicle.make,
    model: vehicle.model,
    year: vehicle.year ?? '',
    vehicle_type: vehicle.vehicle_type as VehicleType,
    fuel_type: vehicle.fuel_type as FuelType,
    default_rate_id: vehicle.default_rate_id ?? null,
    create_rate: false,
    new_rate: newRate,
    is_default: vehicle.is_default,
    notes: vehicle.notes,
  };
}

function yearValue(year: number | string): number | null {
  const text = String(year).trim();
  return text === '' ? null : Number(text);
}

const maxLength = (max: number) => z.string().refine((v) => v.trim().length <= max, `Use ${max} characters or fewer.`);

/** Client-side checks (the API repeats them); new-rate errors use the API's `new_rate.*` names. */
export const vehicleSchema = z
  .object({
    name: z.string().trim().min(1, 'Enter a name for the vehicle.').max(80, 'Use 80 characters or fewer.'),
    registration: maxLength(12),
    make: maxLength(60),
    model: maxLength(60),
    year: z.unknown().refine((v) => {
      const year = yearValue(v as number | string);
      return year === null || (Number.isInteger(year) && year >= 1900 && year <= 2100);
    }, 'Enter a year between 1900 and 2100.'),
    notes: maxLength(1000),
  })
  .loose()
  .superRefine((raw, ctx) => {
    const values = raw as unknown as VehicleFormValues;
    if (!values.create_rate) return;
    for (const issue of newRateSchema.safeParse(values.new_rate).error?.issues ?? []) {
      ctx.addIssue({ code: 'custom', path: ['new_rate', ...issue.path], message: issue.message });
    }
  });

export function toVehicleInput(values: VehicleFormValues): VehicleInput {
  return {
    name: values.name.trim(),
    registration: values.registration.trim(),
    make: values.make.trim(),
    model: values.model.trim(),
    year: yearValue(values.year),
    vehicle_type: values.vehicle_type,
    fuel_type: values.fuel_type,
    default_rate_id: values.create_rate ? null : values.default_rate_id,
    new_rate: values.create_rate ? toNewRateInput(values.new_rate) : null,
    is_default: values.is_default,
    notes: values.notes.trim(),
  };
}

/** The full input for an existing vehicle, made the one new trips start with. */
export function makeDefaultInput(vehicle: Vehicle): VehicleInput {
  return { ...toVehicleInput(fromVehicle(vehicle)), is_default: true };
}
