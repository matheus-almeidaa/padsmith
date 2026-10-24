export type Target =
  | { kind: 'gamepad' | 'key' | 'mouse' | 'trigger'; code: string }
  | { kind: 'dpad'; code: string; value: number }
  | { kind: 'disabled' };
export function makeTarget(kind: Target['kind'], code?: string): Target {
  if (kind === 'disabled') return { kind };
  if (kind === 'dpad') {
    const [axis, value] = (code ?? 'ABS_HAT0Y:-1').split(':');
    return { kind, code: axis, value: Number(value ?? -1) };
  }
  return {
    kind,
    code:
      code ??
      (kind === 'gamepad'
        ? 'BTN_SOUTH'
        : kind === 'key'
          ? 'KEY_SPACE'
          : kind === 'trigger'
            ? 'ABS_Z'
            : 'BTN_LEFT'),
  };
}
export function defaultTarget(code: string): Target {
  const dpad: Record<string, string> = {
    BTN_DPAD_UP: 'ABS_HAT0Y:-1',
    BTN_DPAD_DOWN: 'ABS_HAT0Y:1',
    BTN_DPAD_LEFT: 'ABS_HAT0X:-1',
    BTN_DPAD_RIGHT: 'ABS_HAT0X:1',
  };
  if (dpad[code]) return makeTarget('dpad', dpad[code]);
  if (code === 'BTN_TL2' || code === 'BTN_TR2')
    return makeTarget('trigger', code === 'BTN_TL2' ? 'ABS_Z' : 'ABS_RZ');
  return buttons.includes(code) ? makeTarget('gamepad', code) : { kind: 'disabled' };
}
export function targetFromCode(code: string): Target | null {
  if (!code) return null;
  if (code === 'ABS_Z' || code === 'ABS_RZ') return makeTarget('trigger', code);
  if (code.startsWith('ABS_HAT')) return makeTarget('dpad', code);
  if (['BTN_LEFT', 'BTN_RIGHT', 'BTN_MIDDLE', 'BTN_SIDE', 'BTN_EXTRA'].includes(code))
    return makeTarget('mouse', code);
  if (code.startsWith('BTN_DPAD')) return defaultTarget(code);
  return makeTarget(code.startsWith('BTN_') ? 'gamepad' : 'key', code);
}
export function targetLabel(target: Target, layout = 'xbox'): string {
  if (target.kind === 'disabled') return '—';
  if (target.kind === 'dpad')
    return target.code === 'ABS_HAT0X'
      ? target.value < 0
        ? '←'
        : '→'
      : target.value < 0
        ? '↑'
        : '↓';
  return label(target.code, layout);
}
export interface Binding {
  output?: Target | null;
  mode?: 'hold' | 'toggle' | 'turbo';
  hz?: number;
}
export interface AxisBinding {
  mode: 'gamepad' | 'keys' | 'mouse';
  target?: string | null;
  invert: boolean;
  deadzone: number;
  curve: number;
  threshold: number;
  sensitivity: number;
  negative?: Target | null;
  positive?: Target | null;
}
export interface Profile {
  version: number;
  id: string;
  name: string;
  layout: string;
  buttons: Record<string, Binding>;
  axes: Record<string, AxisBinding>;
  macros?: Record<string, Macro>;
  triggers?: MacroTrigger[];
  processes?: string[];
  devices?: string[];
  priority?: number;
  layers?: Record<string, Layer>;
  emergency?: Emergency;
  shortcuts?: ProfileShortcut[];
  aliases?: Record<string, string>;
}
export interface Layer {
  name: string;
  activation: string[];
  priority: number;
  buttons: Record<string, Binding>;
  axes: Record<string, AxisBinding>;
}
export interface Emergency {
  enabled: boolean;
  buttons: string[];
  hold_ms: number;
}
export interface ProfileShortcut {
  buttons: string[];
  profile: string;
  hold_ms: number;
}
export type MacroStep =
  | { action: 'press' | 'release'; target: Target }
  | { action: 'hold'; target: Target; ms: number }
  | { action: 'wait'; ms: number }
  | { action: 'repeat'; count: number; steps: MacroStep[] }
  | { action: 'axis'; code: string; value: number }
  | { action: 'move'; x: number; y: number };
export interface Macro {
  name: string;
  steps: MacroStep[];
}
export interface MacroTrigger {
  id: string;
  buttons: string[];
  macro_id: string;
  kind: 'tap' | 'long_press' | 'double_tap' | 'chord' | 'while_held' | 'toggle_loop';
  threshold_ms: number;
  window_ms: number;
  consume: boolean;
}
export interface NodeInfo {
  path: string;
  role: string;
  name: string;
  primary: boolean;
  keys?: string[];
  axes?: Record<string, { minimum: number; maximum: number }>;
}
export interface DeviceInfo {
  id: string;
  name: string;
  vendor: string;
  product: string;
  driver: string;
  serial: string;
  physical_path: string;
  connection?: string;
  nodes: NodeInfo[];
  hidraw: string[];
  battery: number | null;
  hidraw_hidden: boolean;
}
export interface Controller {
  device: DeviceInfo;
  profile: string;
  paused: boolean;
  error: string | null;
  recording?: boolean;
  automatic?: boolean;
  layers?: string[];
}
export interface Status {
  controllers: Controller[];
  profiles: Profile[];
  version: string;
  processes?: string[];
}
export interface RawEvent {
  event: 'raw';
  device: string;
  node: string;
  role: string;
  time_us: number;
  event_type: number;
  code: number;
  value: number;
  label: string;
}
export type Message =
  | { event: 'recording'; recording: { sequence: Macro; omitted: string[] } }
  | { event: 'status'; status: Status }
  | RawEvent
  | { event: 'ok' }
  | { event: 'error' | 'notice'; message: string }
  | { event: 'dropped'; count: number };
export type Command =
  | { command: 'record_start' | 'record_stop'; device: string }
  | { command: 'run_macro'; device: string; macro_id: string }
  | { command: 'status' }
  | { command: 'save_profile'; profile: Profile }
  | { command: 'apply'; device: string | null; profile: string }
  | { command: 'pause'; device: string | null; paused: boolean };
export interface Check {
  subject: string;
  level: string;
  detail: string;
}
export const buttons = [
  'BTN_SOUTH',
  'BTN_EAST',
  'BTN_WEST',
  'BTN_NORTH',
  'BTN_TL',
  'BTN_TR',
  'BTN_SELECT',
  'BTN_START',
  'BTN_MODE',
  'BTN_THUMBL',
  'BTN_THUMBR',
];
export const axes = [
  'ABS_X',
  'ABS_Y',
  'ABS_RX',
  'ABS_RY',
  'ABS_Z',
  'ABS_RZ',
  'ABS_HAT0X',
  'ABS_HAT0Y',
];
export function label(code: string, layout = 'xbox'): string {
  const face: Record<string, string[]> = {
    BTN_SOUTH: ['A', '×', 'B'],
    BTN_EAST: ['B', '○', 'A'],
    BTN_WEST: ['X', '□', 'Y'],
    BTN_NORTH: ['Y', '△', 'X'],
    BTN_TL: ['LB', 'L1', 'L'],
    BTN_TR: ['RB', 'R1', 'R'],
    BTN_SELECT: ['Select', 'Share', '−'],
    BTN_START: ['Start', 'Options', '+'],
    BTN_MODE: ['Home', 'PS', 'Home'],
    ABS_Z: ['LT', 'L2', 'ZL'],
    ABS_RZ: ['RT', 'R2', 'ZR'],
  };
  const index = layout === 'playstation' ? 1 : layout === 'nintendo' ? 2 : 0;
  return (
    face[code]?.[index] ??
    (
      {
        BTN_TL: 'LB',
        BTN_TR: 'RB',
        BTN_SELECT: 'Select',
        BTN_START: 'Start',
        BTN_MODE: 'Home',
        BTN_THUMBL: 'L3',
        BTN_THUMBR: 'R3',
        rear_l4: 'L4',
        rear_r4: 'R4',
      } as Record<string, string>
    )[code] ??
    code
  );
}
export const defaultProfile = (): Profile => ({
  version: 1,
  id: 'default',
  name: 'Padrão',
  layout: 'xbox',
  buttons: {},
  axes: {},
});
export const defaultAxis = (): AxisBinding => ({
  mode: 'gamepad',
  invert: false,
  deadzone: 0,
  curve: 1,
  threshold: 0.5,
  sensitivity: 800,
});
