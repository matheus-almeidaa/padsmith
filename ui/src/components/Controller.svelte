<script lang="ts">
  import { label } from '../lib/types';
  export let selected = '';
  function emptyButtons() {
    return new Set<string>();
  }
  export let active: Set<string> = emptyButtons();
  export let layout = 'xbox';
  export let axes: Record<string, number> = {};
  export let aliases: Record<string, string> = {};
  export let choose: (code: string) => void;
  const face = [
    { code: 'BTN_NORTH', x: 460, y: 133 },
    { code: 'BTN_WEST', x: 426, y: 168 },
    { code: 'BTN_EAST', x: 494, y: 168 },
    { code: 'BTN_SOUTH', x: 460, y: 202 },
  ];
  const key = (event: KeyboardEvent, code: string) => {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      choose(code);
    }
  };
</script>

<svg
  class="controller"
  viewBox="0 0 640 420"
  aria-label="Controle interativo com disposição Xbox e botões traseiros"
  role="group"
>
  <defs>
    <linearGradient id="shell" x1="0" y1="0" x2=".4" y2="1"
      ><stop stop-color="#555a60" /><stop offset=".38" stop-color="#373b40" /><stop
        offset="1"
        stop-color="#25282c"
      /></linearGradient
    >
    <linearGradient id="grip" x1="0" y1="0" x2="1" y2="1"
      ><stop stop-color="#303439" /><stop offset="1" stop-color="#1b1e22" /></linearGradient
    >
    <radialGradient id="stick"
      ><stop stop-color="#34383c" /><stop offset=".8" stop-color="#202327" /><stop
        offset="1"
        stop-color="#41464b"
      /></radialGradient
    >
    <linearGradient id="button" x2="0" y2="1"
      ><stop stop-color="#42474d" /><stop offset="1" stop-color="#25292e" /></linearGradient
    >
    <filter id="shadow" x="-40%" y="-40%" width="180%" height="200%"
      ><feDropShadow
        dx="0"
        dy="25"
        stdDeviation="20"
        flood-color="#000"
        flood-opacity=".35"
      /></filter
    >
    <pattern id="texture" width="5" height="5" patternUnits="userSpaceOnUse"
      ><circle cx="2" cy="2" r=".6" fill="#8c949b" opacity=".16" /></pattern
    >
  </defs>
  <ellipse cx="320" cy="374" rx="220" ry="17" fill="currentColor" opacity=".04" />
  <g filter="url(#shadow)">
    <g
      class="hit"
      class:pressed={(axes.ABS_Z ?? 0) > 0.01 || active.has('BTN_TL2')}
      role="button"
      tabindex="0"
      aria-label={'Configurar ' + label('ABS_Z', layout)}
      on:click={() => choose('trigger_left')}
      on:keydown={(e) => key(e, 'trigger_left')}
      ><rect x="162" y="37" width="80" height="17" rx="5" fill="#30363b" stroke="#60686e" /><text
        x="202"
        y="49"
        class="small">{label('ABS_Z', layout)}</text
      ></g
    >
    <g
      class="hit"
      class:pressed={(axes.ABS_RZ ?? 0) > 0.01 || active.has('BTN_TR2')}
      role="button"
      tabindex="0"
      aria-label={'Configurar ' + label('ABS_RZ', layout)}
      on:click={() => choose('trigger_right')}
      on:keydown={(e) => key(e, 'trigger_right')}
      ><rect x="398" y="37" width="80" height="17" rx="5" fill="#30363b" stroke="#60686e" /><text
        x="438"
        y="49"
        class="small">{label('ABS_RZ', layout)}</text
      ></g
    >
    <path
      d="M148 94c-1-27 9-41 39-41h61l12 40m232 1c1-27-9-41-39-41h-61l-12 40"
      fill="#22262a"
      stroke="#555a60"
      stroke-width="1.5"
    />
    <path d="M161 73h89M390 73h89" stroke="#737a81" opacity=".5" />
    <path
      d="M165 81h311c25 0 42 19 50 44l49 172c14 54-34 89-70 53l-66-66H201l-66 66c-36 36-84 1-70-53l49-172c8-25 26-44 51-44Z"
      fill="url(#shell)"
      stroke="#60666b"
      stroke-width="1.3"
    />
    <path d="m115 166-47 139c-8 32 15 61 41 49l96-105Z" fill="url(#grip)" />
    <path d="m525 166 47 139c8 32-15 61-41 49l-96-105Z" fill="url(#grip)" />
    <path
      d="m115 166-47 139c-8 32 15 61 41 49l96-105Zm410 0 47 139c8 32-15 61-41 49l-96-105Z"
      fill="url(#texture)"
    />
    <path d="M164 88h312c21 0 37 16 45 40" fill="none" stroke="#a9afb5" opacity=".17" />
    <path d="m124 249-26 79m418-79 26 79" stroke="#747d83" opacity=".15" stroke-width="2" />
    <g
      class:chosen={selected === 'BTN_TL'}
      class:pressed={active.has('BTN_TL')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label={'Configurar ' + label('BTN_TL', layout)}
      on:click={() => choose('BTN_TL')}
      on:keydown={(e) => key(e, 'BTN_TL')}
      ><rect x="155" y="62" width="95" height="22" rx="9" fill="#3e4449" /><text
        x="202"
        y="78"
        class="small">{label('BTN_TL', layout)}</text
      ></g
    >
    <g
      class:chosen={selected === 'BTN_TR'}
      class:pressed={active.has('BTN_TR')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label={'Configurar ' + label('BTN_TR', layout)}
      on:click={() => choose('BTN_TR')}
      on:keydown={(e) => key(e, 'BTN_TR')}
      ><rect x="390" y="62" width="95" height="22" rx="9" fill="#3e4449" /><text
        x="437"
        y="78"
        class="small">{label('BTN_TR', layout)}</text
      ></g
    >
    <g
      class:chosen={selected === 'BTN_MODE'}
      class:pressed={active.has('BTN_MODE')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar Home"
      on:click={() => choose('BTN_MODE')}
      on:keydown={(e) => key(e, 'BTN_MODE')}
      ><circle cx="320" cy="115" r="18" fill="#23272b" stroke="#5d646a" /><path
        d="m311 108 18 14m-18 0 18-14"
        stroke="#d0d8c8"
        stroke-width="2"
      /></g
    >
    <g
      class:chosen={selected === 'BTN_SELECT'}
      class:pressed={active.has('BTN_SELECT')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar Select"
      on:click={() => choose('BTN_SELECT')}
      on:keydown={(e) => key(e, 'BTN_SELECT')}
      ><circle cx="278" cy="163" r="13" fill="url(#button)" stroke="#555c62" /><path
        d="M273 160h7v6h-7Zm3-3h7v6"
        fill="none"
        stroke="#aab1b6"
      /></g
    >
    <g
      class:chosen={selected === 'BTN_START'}
      class:pressed={active.has('BTN_START')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar Start"
      on:click={() => choose('BTN_START')}
      on:keydown={(e) => key(e, 'BTN_START')}
      ><circle cx="362" cy="163" r="13" fill="url(#button)" stroke="#555c62" /><path
        d="M357 159h10m-10 4h10m-10 4h10"
        stroke="#aab1b6"
      /></g
    >
    <g
      class:chosen={selected === 'BTN_THUMBL'}
      class:pressed={active.has('BTN_THUMBL')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar analógico esquerdo L3"
      on:click={() => choose('BTN_THUMBL')}
      on:keydown={(e) => key(e, 'BTN_THUMBL')}
      ><circle cx="183" cy="162" r="49" fill="#202327" stroke="#61686e" /><circle
        cx="183"
        cy="162"
        r="38"
        fill="url(#stick)"
        stroke="#14171a"
        stroke-width="4"
      /><circle cx="183" cy="162" r="30" fill="none" stroke="#535960" stroke-dasharray="1 3" /></g
    >
    <g
      class:chosen={selected === 'BTN_THUMBR'}
      class:pressed={active.has('BTN_THUMBR')}
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar analógico direito R3"
      on:click={() => choose('BTN_THUMBR')}
      on:keydown={(e) => key(e, 'BTN_THUMBR')}
      ><circle cx="385" cy="257" r="48" fill="#202327" stroke="#5e666d" /><circle
        cx="385"
        cy="257"
        r="37"
        fill="url(#stick)"
        stroke="#14171a"
        stroke-width="4"
      /><circle cx="385" cy="257" r="29" fill="none" stroke="#535960" stroke-dasharray="1 3" /></g
    >
    <g
      class="hit"
      role="button"
      tabindex="0"
      aria-label="Configurar d-pad"
      class:pressed={(axes.ABS_HAT0X ?? 0) !== 0 ||
        (axes.ABS_HAT0Y ?? 0) !== 0 ||
        ['BTN_DPAD_UP', 'BTN_DPAD_DOWN', 'BTN_DPAD_LEFT', 'BTN_DPAD_RIGHT'].some((c) =>
          active.has(c),
        )}
      on:click={() => choose('dpad')}
      on:keydown={(e) => key(e, 'dpad')}
      ><circle cx="249" cy="257" r="43" fill="#272b30" stroke="#50575d" /><path
        d="M237 225h24v20h20v24h-20v20h-24v-20h-20v-24h20Z"
        fill="#383e44"
        stroke="#111518"
        stroke-width="2"
      /><path
        d="m244 237 5-5 5 5m-22 15-5 5 5 5m34-10 5 5-5 5m-22 15 5 5 5-5"
        fill="none"
        stroke="#787f86"
      /></g
    >
    {#each face as b}
      <g
        class:chosen={selected === b.code}
        class:pressed={active.has(b.code)}
        class="hit face"
        role="button"
        tabindex="0"
        aria-label={'Configurar ' + label(b.code, layout)}
        on:click={() => choose(b.code)}
        on:keydown={(e) => key(e, b.code)}
      >
        <circle cx={b.x} cy={b.y} r="22" fill="#1c2024" />
        <circle class="cap" cx={b.x} cy={b.y - 1} r="18" fill="url(#button)" stroke="#565d64" />
        <text x={b.x} y={b.y + 6} class:west={b.code === 'BTN_WEST'}>{label(b.code, layout)}</text>
      </g>
    {/each}
    <path d="M308 193h24" stroke="#747d84" stroke-linecap="round" stroke-width="4" />
  </g>
  <g
    class:chosen={selected === 'rear_l4' || selected === aliases.rear_l4}
    class:pressed={active.has(aliases.rear_l4 ?? '')}
    class="hit rear"
    role="button"
    tabindex="0"
    aria-label="Identificar botão traseiro L4"
    on:click={() => choose('rear_l4')}
    on:keydown={(e) => key(e, 'rear_l4')}
    ><path
      d="m163 310 20-25c6-7 16-2 13 7l-11 31c-4 10-17 8-22 2Z"
      fill="#41494a"
      stroke="#899a7b"
    /><text x="180" y="348">L4</text><path d="m180 331 0 6" stroke="#7b8582" /></g
  >
  <g
    class:chosen={selected === 'rear_r4' || selected === aliases.rear_r4}
    class:pressed={active.has(aliases.rear_r4 ?? '')}
    class="hit rear"
    role="button"
    tabindex="0"
    aria-label="Identificar botão traseiro R4"
    on:click={() => choose('rear_r4')}
    on:keydown={(e) => key(e, 'rear_r4')}
    ><path
      d="m477 310-20-25c-6-7-16-2-13 7l11 31c4 10 17 8 22 2Z"
      fill="#41494a"
      stroke="#899a7b"
    /><text x="460" y="348">R4</text><path d="m460 331 0 6" stroke="#7b8582" /></g
  >
</svg>

<style>
  .controller {
    width: 100%;
    max-width: 730px;
    overflow: visible;
    color: var(--text);
  }
  text {
    font:
      600 17px 'Inter',
      sans-serif;
    text-anchor: middle;
    fill: #c9ced2;
    pointer-events: none;
  }
  .small {
    font-size: 11px;
    letter-spacing: 1px;
  }
  .hit {
    cursor: pointer;
    outline: none;
  }
  .hit > * {
    transition:
      stroke 0.15s,
      fill 0.15s,
      filter 0.15s;
  }
  .hit:hover > circle,
  .hit:focus > circle,
  .chosen > circle,
  .chosen > rect,
  .chosen > path,
  .hit:hover > rect {
    stroke: #baf266;
    stroke-width: 2;
  }
  .pressed > .cap,
  .pressed > rect,
  .pressed > circle:first-child {
    fill: #baf266;
    stroke: #d0ff8a;
  }
  .pressed text {
    fill: #192214;
  }
  .chosen text {
    fill: #baf266;
  }
  .rear text {
    font:
      500 12px 'Inter',
      sans-serif;
    fill: #9ba698;
    letter-spacing: 1px;
  }
  .rear.pressed > path:first-child {
    fill: #baf266;
    stroke: #d0ff8a;
  }
  .face text.west {
    fill: #baf266;
  }
  .face.pressed text.west {
    fill: #192214;
  }
  .hit:focus {
    filter: drop-shadow(0 0 3px #baf266);
  }
</style>
