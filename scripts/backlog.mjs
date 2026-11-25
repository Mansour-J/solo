#!/usr/bin/env node
// Reads and checks the backlog data embedded in docs/backlog/backlog.html.
//
//   npm run backlog -- check                 validate the data (part of `npm run check`)
//   npm run backlog -- next <section>        next free ID in a section (e.g. phase-7, foundation)
//   npm run backlog -- show SL-123           print one item as JSON
//   npm run backlog -- list [filters]        one line per item; filters: status=Ready,Idea type=Feature
//                                            priority=P1 size=M section=phase-5 open (hides Done/Dropped)
//
// How to add and update items: .claude/skills/backlog/SKILL.md.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const file = path.join(root, 'docs/backlog/backlog.html');
const START = /<script type="application\/json" id="backlog-data">/;
const END = '</script>';

function load() {
  const html = fs.readFileSync(file, 'utf8');
  const start = html.search(START);
  if (start < 0) throw new Error('backlog.html has no <script type="application/json" id="backlog-data"> block');
  const bodyStart = html.indexOf('>', start) + 1;
  const bodyEnd = html.indexOf(END, bodyStart);
  try {
    return JSON.parse(html.slice(bodyStart, bodyEnd));
  } catch (error) {
    throw new Error(`backlog-data is not valid JSON: ${error.message}`, { cause: error });
  }
}

const ITEM_KEYS = [
  'id',
  'title',
  'section',
  'status',
  'statusNote',
  'type',
  'priority',
  'size',
  'dependsOn',
  'summary',
  'criteria',
  'code',
  'notes',
  'labels',
];

function check(data) {
  const errors = [];
  const warnings = [];
  const names = (list) => new Set((list || []).map((x) => x.name));
  const statuses = names(data.statuses);
  const types = names(data.types);
  const priorities = names(data.priorities);
  const sizes = names(data.sizes);
  const labels = names(data.labels);
  const sections = new Map((data.sections || []).map((s) => [s.id, s]));
  const ids = new Map();

  for (const s of data.sections || []) {
    if (!/^\d$/.test(s.idPrefix ?? '')) errors.push(`section ${s.id}: idPrefix must be one digit`);
    if (!Array.isArray(s.intro)) errors.push(`section ${s.id}: intro must be an array of paragraphs`);
  }

  for (const [index, item] of (data.items || []).entries()) {
    const where = item.id || `items[${index}]`;
    const err = (m) => errors.push(`${where}: ${m}`);
    if (!/^SL-\d{3}$/.test(item.id ?? '')) err('id must look like SL-123');
    else if (ids.has(item.id)) err('duplicate id');
    ids.set(item.id, item);
    for (const key of Object.keys(item)) if (!ITEM_KEYS.includes(key)) err(`unknown field "${key}"`);
    if (!item.title?.trim()) err('title is required');
    const section = sections.get(item.section);
    if (!section) err(`section "${item.section}" doesn't exist`);
    else if (item.id && item.id[3] !== section.idPrefix) err(`id should start with SL-${section.idPrefix} in section ${section.id}`);
    if (!statuses.has(item.status)) err(`status "${item.status}" isn't one of ${[...statuses].join(', ')}`);
    if (!types.has(item.type)) err(`type "${item.type}" isn't one of ${[...types].join(', ')}`);
    if (!priorities.has(item.priority)) err(`priority "${item.priority}" isn't one of ${[...priorities].join(', ')}`);
    if (!sizes.has(item.size)) err(`size "${item.size}" isn't one of ${[...sizes].join(', ')}`);
    if (!Array.isArray(item.dependsOn)) err('dependsOn must be an array (empty for none)');
    if ('labels' in item && (!Array.isArray(item.labels) || item.labels.length === 0)) err('labels must be a non-empty array, or left out');
    else for (const l of item.labels ?? []) if (!labels.has(l)) err(`label "${l}" isn't one of ${[...labels].join(', ')}`);
    if (!Array.isArray(item.criteria)) err('criteria must be an array (empty for none)');
    else
      item.criteria.forEach((c, i) => {
        if (typeof c.done !== 'boolean' || !c.text?.trim()) err(`criteria[${i}] needs { "done": boolean, "text": "…" }`);
      });
    for (const key of ['statusNote', 'summary', 'code', 'notes'])
      if (key in item && (typeof item[key] !== 'string' || !item[key].trim())) err(`${key} must be non-empty text, or left out`);
    if (item.status === 'Dropped' && !item.statusNote) err('a Dropped item needs a statusNote saying why');
    if (item.status === 'Blocked' && !item.statusNote && !item.notes) err('a Blocked item needs a statusNote or notes saying why');
    if (item.status === 'Done' && item.criteria?.some((c) => !c.done))
      warnings.push(`${where}: Done with unticked criteria (fine when the rest is a deliberate follow-up)`);
  }

  for (const item of data.items || [])
    for (const dep of item.dependsOn || []) {
      if (!ids.has(dep)) errors.push(`${item.id}: depends on ${dep}, which doesn't exist`);
      if (dep === item.id) errors.push(`${item.id}: depends on itself`);
    }
  return { errors, warnings };
}

function nextId(data, sectionId) {
  const section = data.sections.find((s) => s.id === sectionId);
  if (!section) throw new Error(`No section "${sectionId}". Sections: ${data.sections.map((s) => s.id).join(', ')}`);
  const nums = data.items.filter((i) => i.section === section.id).map((i) => Number(i.id.slice(3)));
  return `SL-${String(Math.max(Number(section.idPrefix) * 100, ...nums) + 1).padStart(3, '0')}`;
}

function list(data, args) {
  const filters = {};
  let open = false;
  for (const arg of args) {
    if (arg === 'open') open = true;
    else {
      const [key, value = ''] = arg.split('=');
      filters[key] = new Set(value.split(','));
    }
  }
  for (const item of data.items) {
    if (open && ['Done', 'Dropped', 'Parked'].includes(item.status)) continue;
    if (Object.entries(filters).some(([k, v]) => !v.has(item[k]))) continue;
    const done = item.criteria.filter((c) => c.done).length;
    const criteria = item.criteria.length ? ` [${done}/${item.criteria.length}]` : '';
    console.log(
      `${item.id}  ${item.status.padEnd(11)} ${item.priority} ${item.type.padEnd(8)} ${item.size.padEnd(2)} ${item.title}${criteria}`,
    );
  }
}

const [command = 'check', ...args] = process.argv.slice(2);
try {
  const data = load();
  if (command === 'check') {
    const { errors, warnings } = check(data);
    for (const w of warnings) console.log(`warning: ${w}`);
    for (const e of errors) console.error(`error: ${e}`);
    if (errors.length) {
      console.error(`docs/backlog/backlog.html: ${errors.length} error(s)`);
      process.exit(1);
    }
    console.log(`docs/backlog/backlog.html: ${data.items.length} items in ${data.sections.length} sections, OK`);
  } else if (command === 'next') {
    console.log(nextId(data, args[0]));
  } else if (command === 'show') {
    const item = data.items.find((i) => i.id === args[0]);
    if (!item) throw new Error(`No item ${args[0]}`);
    console.log(JSON.stringify(item, null, 2));
  } else if (command === 'list') {
    list(data, args);
  } else {
    throw new Error(`Unknown command "${command}". Use check, next <section>, show <id> or list [filters].`);
  }
} catch (error) {
  console.error(error.message);
  process.exit(1);
}
