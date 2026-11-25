// Display helpers for mileage. Values come from the API as decimal strings; they are converted to
// numbers only to format them for people, never to calculate (the API calculates every claim).
import { getNumberLocale } from '@/lib/format';
import type { FuelType, VehicleType } from './api';

export const VEHICLE_TYPE_LABELS: Record<VehicleType, string> = {
  car: 'Car',
  van: 'Van',
  ute: 'Ute',
  motorcycle: 'Motorcycle',
  truck: 'Truck',
  other: 'Other',
};

export const FUEL_TYPE_LABELS: Record<FuelType, string> = {
  petrol: 'Petrol',
  diesel: 'Diesel',
  hybrid: 'Hybrid',
  plug_in_hybrid: 'Plug-in hybrid',
  electric: 'Electric',
  other: 'Other',
};

export const VEHICLE_TYPE_OPTIONS = (Object.keys(VEHICLE_TYPE_LABELS) as VehicleType[]).map((value) => ({
  value,
  label: VEHICLE_TYPE_LABELS[value],
}));
export const FUEL_TYPE_OPTIONS = (Object.keys(FUEL_TYPE_LABELS) as FuelType[]).map((value) => ({ value, label: FUEL_TYPE_LABELS[value] }));

/** Kilometres with one decimal place, e.g. "1,234.5 km". */
export function formatKm(km: string | number | null | undefined): string {
  if (km === null || km === undefined || km === '') return '—';
  const value = Number(km);
  if (!Number.isFinite(value)) return '—';
  return `${new Intl.NumberFormat(getNumberLocale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 }).format(value)} km`;
}

/** A rate per kilometre, e.g. "1.0400" → "$1.04/km", "0.8250" → "$0.825/km". */
export function formatRate(rate: string | number | null | undefined, currency = 'NZD'): string {
  if (rate === null || rate === undefined || rate === '') return '—';
  const value = Number(rate);
  if (!Number.isFinite(value)) return '—';
  const money = new Intl.NumberFormat(getNumberLocale(), {
    style: 'currency',
    currency,
    currencyDisplay: 'narrowSymbol',
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  }).format(value);
  return `${money}/km`;
}

/** IRD's tiers, e.g. "$0.37/km after 14,000 km", or '' without tiers. */
export function formatTiers(limit: string | null | undefined, tier2Rate: string | null | undefined, currency = 'NZD'): string {
  if (!limit || !tier2Rate) return '';
  const km = new Intl.NumberFormat(getNumberLocale(), { maximumFractionDigits: 1 }).format(Number(limit));
  return `${formatRate(tier2Rate, currency)} after ${km} km`;
}

/** A percentage from the API ("62.50") without trailing zeros, e.g. "62.5%". */
export function formatShare(value: string | number | null | undefined): string {
  if (value === null || value === undefined || value === '') return '—';
  const number = Number(value);
  if (!Number.isFinite(number)) return '—';
  return `${new Intl.NumberFormat(getNumberLocale(), { maximumFractionDigits: 2 }).format(number)}%`;
}

/** "Toyota Corolla 2019 · ABC123", skipping empty parts. */
export function vehicleDescription(vehicle: { make: string; model: string; year?: number | null; registration: string }): string {
  const name = [vehicle.make, vehicle.model, vehicle.year ? String(vehicle.year) : ''].filter(Boolean).join(' ');
  return [name, vehicle.registration].filter(Boolean).join(' · ');
}
