import { z } from 'zod';
import type { FrequentTrip, Trip, TripInput, Vehicle } from './api';

export type DistanceMode = 'distance' | 'odometer';

/** `''` uses the vehicle's default rate, `custom` a rate entered for this trip, anything else a rate id. */
export const VEHICLE_RATE = '';
export const CUSTOM_RATE = 'custom';

/** Form values. Field names match `TripInput` so server errors land on the right input. */
export interface TripFormValues {
  vehicle_id: string | null;
  trip_date: string | null;
  start_location: string;
  destination: string;
  purpose: string;
  /** Not sent: whether the distance comes from odometer readings. */
  distance_mode: DistanceMode;
  start_odometer: string;
  end_odometer: string;
  distance_km: string;
  /** There and back: `distance_km` is one way (distance mode only). */
  return_trip: boolean;
  business_percentage: string;
  /** Not sent: which rate to use (see `VEHICLE_RATE` and `CUSTOM_RATE`). */
  rate_id: string;
  rate_per_km: string;
  notes: string;
}

/** The vehicle new trips start with: the default one, else the first active one. */
export function defaultVehicle(vehicles: Vehicle[]): Vehicle | undefined {
  const active = vehicles.filter((v) => !v.archived);
  return active.find((v) => v.is_default) ?? active[0];
}

export function emptyTrip(today: string, vehicleId: string | null): TripFormValues {
  return {
    vehicle_id: vehicleId,
    trip_date: today,
    start_location: '',
    destination: '',
    purpose: '',
    distance_mode: 'distance',
    start_odometer: '',
    end_odometer: '',
    distance_km: '',
    return_trip: false,
    business_percentage: '100',
    rate_id: VEHICLE_RATE,
    rate_per_km: '',
    notes: '',
  };
}

/** Drops trailing zeros the API adds to decimals: "100.00" → "100", "42.5" stays. */
function plain(value: string | null | undefined): string {
  if (!value) return '';
  return value.includes('.') ? value.replace(/\.?0+$/, '') : value;
}

export function fromTrip(trip: Trip): TripFormValues {
  const odometer = trip.start_odometer != null && trip.end_odometer != null;
  return {
    vehicle_id: trip.vehicle_id,
    trip_date: trip.trip_date,
    start_location: trip.start_location,
    destination: trip.destination,
    purpose: trip.purpose,
    distance_mode: odometer ? 'odometer' : 'distance',
    start_odometer: odometer ? plain(trip.start_odometer) : '',
    end_odometer: odometer ? plain(trip.end_odometer) : '',
    // A return trip is entered one way; the API keeps the whole distance.
    distance_km: odometer ? '' : plain(trip.return_trip ? (Number(trip.distance_km) / 2).toFixed(1) : trip.distance_km),
    return_trip: trip.return_trip,
    business_percentage: plain(trip.business_percentage),
    rate_id: trip.rate_overridden ? CUSTOM_RATE : (trip.rate_id ?? VEHICLE_RATE),
    rate_per_km: trip.rate_overridden ? plain(trip.rate_per_km) : '',
    notes: trip.notes,
  };
}

const KM = /^\d{1,7}(\.\d)?$/;
const PERCENT = /^\d{1,3}(\.\d{1,2})?$/;
const RATE = /^\d{1,3}(\.\d{1,4})?$/;

/** A decimal typed by a person, with spaces and thousands separators removed. */
function decimal(value: string): string {
  return value.replace(/[\s,]/g, '');
}

// Comparing readings as numbers is only a hint for the person typing; the API decides.
function validKm(value: string): boolean {
  return KM.test(decimal(value));
}

export function validPercent(value: string): boolean {
  const text = decimal(value);
  return PERCENT.test(text) && Number(text) <= 100;
}

function validRate(value: string): boolean {
  const text = decimal(value);
  return RATE.test(text) && Number(text) <= 100;
}

export const tripSchema = z
  .object({
    vehicle_id: z
      .string()
      .nullable()
      .refine((v) => !!v, 'Choose a vehicle.'),
    trip_date: z
      .string()
      .nullable()
      .refine((v) => !!v, 'Enter the date of the trip.'),
    purpose: z.string().trim().min(1, 'Enter the purpose of the trip.').max(300, 'Use 300 characters or fewer.'),
    start_location: z.string().max(200, 'Use 200 characters or fewer.'),
    destination: z.string().max(200, 'Use 200 characters or fewer.'),
    notes: z.string().max(2000, 'Use 2,000 characters or fewer.'),
  })
  .loose()
  .superRefine((raw, ctx) => {
    const v = raw as unknown as TripFormValues;
    if (v.distance_mode === 'odometer') {
      if (!validKm(v.start_odometer))
        ctx.addIssue({ code: 'custom', path: ['start_odometer'], message: 'Enter the reading in km, e.g. 10234.5.' });
      if (!validKm(v.end_odometer)) {
        ctx.addIssue({ code: 'custom', path: ['end_odometer'], message: 'Enter the reading in km, e.g. 10276.0.' });
      } else if (validKm(v.start_odometer) && Number(decimal(v.end_odometer)) <= Number(decimal(v.start_odometer))) {
        ctx.addIssue({ code: 'custom', path: ['end_odometer'], message: 'The end reading must be more than the start reading.' });
      }
    } else if (!validKm(v.distance_km) || Number(decimal(v.distance_km)) <= 0) {
      ctx.addIssue({ code: 'custom', path: ['distance_km'], message: 'Enter the distance in km, e.g. 42.5.' });
    }
    if (!validPercent(v.business_percentage)) {
      ctx.addIssue({ code: 'custom', path: ['business_percentage'], message: 'Enter a percentage between 0 and 100.' });
    }
    if (v.rate_id === CUSTOM_RATE && !validRate(v.rate_per_km)) {
      ctx.addIssue({ code: 'custom', path: ['rate_per_km'], message: 'Enter the rate per km, e.g. 1.04.' });
    }
  });

/** The request body. Only the fields for the chosen distance and rate options are sent. */
export function toTripInput(values: TripFormValues): TripInput {
  const odometer = values.distance_mode === 'odometer';
  return {
    vehicle_id: values.vehicle_id ?? '',
    trip_date: values.trip_date ?? '',
    start_location: values.start_location.trim(),
    destination: values.destination.trim(),
    purpose: values.purpose.trim(),
    start_odometer: odometer ? decimal(values.start_odometer) : null,
    end_odometer: odometer ? decimal(values.end_odometer) : null,
    distance_km: odometer ? null : decimal(values.distance_km),
    return_trip: !odometer && values.return_trip,
    business_percentage: decimal(values.business_percentage),
    rate_id: values.rate_id && values.rate_id !== CUSTOM_RATE ? values.rate_id : null,
    rate_per_km: values.rate_id === CUSTOM_RATE ? decimal(values.rate_per_km) : null,
    notes: values.notes.trim(),
  };
}

/** The body for a live calculation, or null until the distance, share and rate are complete. */
export function calculationInput(values: TripFormValues): TripInput | null {
  if (!values.vehicle_id || !values.trip_date) return null;
  const distanceOk =
    values.distance_mode === 'odometer'
      ? validKm(values.start_odometer) && validKm(values.end_odometer)
      : validKm(values.distance_km) && Number(decimal(values.distance_km)) > 0;
  if (!distanceOk || !validPercent(values.business_percentage)) return null;
  if (values.rate_id === CUSTOM_RATE && !validRate(values.rate_per_km)) return null;
  return { ...toTripInput(values), purpose: '', start_location: '', destination: '', notes: '' };
}

/** A new trip filled in from a frequent one: its vehicle, route, purpose, distance and share, dated `date`. */
export function fromFrequent(values: TripFormValues, frequent: FrequentTrip): TripFormValues {
  return {
    ...values,
    vehicle_id: frequent.vehicle_id,
    start_location: frequent.start_location,
    destination: frequent.destination,
    purpose: frequent.purpose,
    distance_mode: 'distance',
    start_odometer: '',
    end_odometer: '',
    distance_km: plain(frequent.distance_km),
    return_trip: frequent.return_trip,
    business_percentage: plain(frequent.business_percentage),
  };
}

/** "Client site · Home → Office · 20 km each way" for the frequent-trip picker. */
export function frequentLabel(frequent: FrequentTrip): string {
  const route = [frequent.start_location, frequent.destination].filter(Boolean).join(' → ');
  const km = `${plain(frequent.distance_km)} km${frequent.return_trip ? ' each way' : ''}`;
  return [frequent.purpose, route, km].filter(Boolean).join(' · ');
}
