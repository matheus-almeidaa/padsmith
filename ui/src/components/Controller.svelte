<script lang="ts">
  import { label } from '../lib/types';
  import darkController from '../assets/controllers/controller-dark.webp';
  import lightController from '../assets/controllers/controller-light.webp';

  export let selected = '';
  function emptyButtons() {
    return new Set<string>();
  }
  export let active: Set<string> = emptyButtons();
  export let layout = 'xbox';
  export let axes: Record<string, number> = {};
  export let aliases: Record<string, string> = {};
  export let choose: (code: string) => void;

  // Coordinates follow the source images, so artwork and hit areas scale together.
  const controls = [
    {
      code: 'trigger_left',
      name: 'ABS_Z',
      x: 402,
      y: 55,
      path: 'M310 83 Q320 56 349 49 L435 29 Q473 21 487 52 L490 61 Q394 73 310 91Z',
    },
    {
      code: 'trigger_right',
      name: 'ABS_RZ',
      x: 1134,
      y: 55,
      path: 'M1226 83 Q1216 56 1187 49 L1101 29 Q1063 21 1049 52 L1046 61 Q1142 73 1226 91Z',
    },
    {
      code: 'BTN_TL',
      x: 402,
      y: 96,
      path: 'M303 94 Q380 71 472 63 Q489 62 492 81 L489 94 L310 136 Q291 140 295 120Z',
    },
    {
      code: 'BTN_TR',
      x: 1134,
      y: 96,
      path: 'M1233 94 Q1156 71 1064 63 Q1047 62 1044 81 L1047 94 L1226 136 Q1245 140 1241 120Z',
    },
    { code: 'BTN_THUMBL', x: 391, y: 336, radius: 103 },
    {
      code: 'BTN_SELECT',
      x: 624,
      y: 280,
      path: 'M607 255 H640 a25 25 0 0 1 0 50 H607 a25 25 0 0 1 0 -50Z',
    },
    { code: 'BTN_MODE', x: 768, y: 277, radius: 51 },
    {
      code: 'BTN_START',
      x: 912,
      y: 280,
      path: 'M895 255 H928 a25 25 0 0 1 0 50 H895 a25 25 0 0 1 0 -50Z',
    },
    { code: 'BTN_NORTH', x: 1156, y: 254, radius: 50 },
    { code: 'BTN_WEST', x: 1065, y: 346, radius: 50 },
    { code: 'BTN_EAST', x: 1248, y: 346, radius: 50 },
    { code: 'BTN_SOUTH', x: 1156, y: 438, radius: 50 },
    {
      code: 'dpad',
      x: 574,
      y: 541,
      path: 'M556 427 H592 Q615 427 615 452 V503 H660 Q684 503 684 527 V557 Q684 581 660 581 H615 V631 Q615 655 591 655 H557 Q533 655 533 631 V581 H487 Q463 581 463 557 V527 Q463 503 487 503 H533 V452 Q533 427 556 427Z',
    },
    { code: 'BTN_THUMBR', x: 965, y: 535, radius: 104 },
  ];
  const rearButtons = [
    {
      code: 'rear_l4',
      x: 430,
      path: 'M375 822 L290 922 Q278 938 278 962 V1080 Q278 1118 316 1118 H384',
    },
    {
      code: 'rear_r4',
      x: 1370,
      path: 'M1425 822 L1510 922 Q1522 938 1522 962 V1080 Q1522 1118 1484 1118 H1416',
    },
  ];
  const faceButtons = ['BTN_NORTH', 'BTN_WEST', 'BTN_EAST', 'BTN_SOUTH'];
  let hovered = '';
  $: focusedCode = hovered || selected;
  $: focusedLabel = controlLabel(focusedCode, layout, aliases);

  function controlLabel(code: string, currentLayout: string, sources: Record<string, string>) {
    if (code === 'trigger_left' || code === 'BTN_TL2') return label('ABS_Z', currentLayout);
    if (code === 'trigger_right' || code === 'BTN_TR2') return label('ABS_RZ', currentLayout);
    if (code === 'dpad') return 'D-pad';
    const rear = Object.entries(sources).find(([, source]) => source === code)?.[0];
    return label(rear ?? code, currentLayout);
  }
  function isSelected(code: string, selection: string, sources: Record<string, string>) {
    return (
      selection === code ||
      selection === sources[code] ||
      (code === 'trigger_left' && selection === 'BTN_TL2') ||
      (code === 'trigger_right' && selection === 'BTN_TR2')
    );
  }
  function isPressed(
    code: string,
    pressed: Set<string>,
    positions: Record<string, number>,
    sources: Record<string, string>,
  ) {
    if (code === 'trigger_left') return (positions.ABS_Z ?? 0) > 0.01 || pressed.has('BTN_TL2');
    if (code === 'trigger_right') return (positions.ABS_RZ ?? 0) > 0.01 || pressed.has('BTN_TR2');
    if (code === 'dpad')
      return (
        (positions.ABS_HAT0X ?? 0) !== 0 ||
        (positions.ABS_HAT0Y ?? 0) !== 0 ||
        ['BTN_DPAD_UP', 'BTN_DPAD_DOWN', 'BTN_DPAD_LEFT', 'BTN_DPAD_RIGHT'].some((c) =>
          pressed.has(c),
        )
      );
    return pressed.has(sources[code] ?? code);
  }
  function key(event: KeyboardEvent, code: string) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      choose(code);
    }
  }
</script>

<div class="controller-art">
  <svg
    class="controller"
    viewBox="0 0 1800 1250"
    aria-label="Controle interativo com botões traseiros L4 e R4"
    role="group"
  >
    <g class="artwork" transform="translate(132 60)" aria-hidden="true">
      <image class="shell shell-dark" href={darkController} width="1536" height="1024" />
      <image class="shell shell-light" href={lightController} width="1536" height="1024" />
    </g>

    <g transform="translate(132 60)">
      {#each controls as control}
        <g
          class="hit"
          class:chosen={isSelected(control.code, selected, aliases)}
          class:pressed={isPressed(control.code, active, axes, aliases)}
          role="button"
          tabindex="0"
          aria-label={'Configurar ' + controlLabel(control.code, layout, aliases)}
          aria-pressed={isSelected(control.code, selected, aliases)}
          on:click={() => choose(control.code)}
          on:keydown={(event) => key(event, control.code)}
          on:mouseenter={() => (hovered = control.code)}
          on:mouseleave={() => (hovered = '')}
          on:focus={() => (hovered = control.code)}
          on:blur={() => (hovered = '')}
        >
          <title>{controlLabel(control.code, layout, aliases)} · Clique para configurar</title>
          {#if control.radius}
            <circle class="halo" cx={control.x} cy={control.y} r={control.radius + 10} />
            <circle class="button-outline" cx={control.x} cy={control.y} r={control.radius} />
          {:else}
            <path class="halo" d={control.path} />
            <path class="button-outline" d={control.path} />
          {/if}
          {#if layout !== 'xbox' && faceButtons.includes(control.code)}
            <circle class="label-cover" cx={control.x} cy={control.y} r="38" />
            <text class="face-label" x={control.x} y={control.y + 2}
              >{label(control.code, layout)}</text
            >
          {/if}
          {#if ['trigger_left', 'trigger_right', 'BTN_TL', 'BTN_TR'].includes(control.code)}
            <rect
              class="label-cover"
              x={control.x - 34}
              y={control.y - 16}
              width="68"
              height="32"
              rx="10"
            />
            <text class="shoulder-label" x={control.x} y={control.y + 1}
              >{label(control.name ?? control.code, layout)}</text
            >
          {/if}
        </g>
      {/each}
    </g>

    {#each rearButtons as rear}
      <g
        class="rear"
        class:chosen={isSelected(rear.code, selected, aliases)}
        class:pressed={isPressed(rear.code, active, axes, aliases)}
      >
        <path class="rear-leader" d={rear.path} />
        <circle class="rear-anchor" cx={rear.code === 'rear_l4' ? 375 : 1425} cy="822" r="7" />
        <g
          class="hit"
          role="button"
          tabindex="0"
          aria-label={'Configurar botão traseiro ' + label(rear.code)}
          aria-pressed={isSelected(rear.code, selected, aliases)}
          on:click={() => choose(rear.code)}
          on:keydown={(event) => key(event, rear.code)}
          on:mouseenter={() => (hovered = rear.code)}
          on:mouseleave={() => (hovered = '')}
          on:focus={() => (hovered = rear.code)}
          on:blur={() => (hovered = '')}
        >
          <title>{label(rear.code)} · Botão na parte de trás do controle</title>
          <circle class="halo" cx={rear.x} cy="1118" r="57" />
          <circle class="rear-cap" cx={rear.x} cy="1118" r="46" />
          <text class="rear-label" x={rear.x} y="1120">{label(rear.code)}</text>
        </g>
      </g>
    {/each}

    <g class="rear-caption" aria-hidden="true">
      <path
        d="M886 1105 H914 M906 1097 L914 1105 L906 1113 M914 1105 V1119 Q914 1131 902 1131 H886"
      />
      <text x="900" y="1171">BOTÕES TRASEIROS</text>
    </g>
  </svg>
  <div class="interaction-hint" aria-hidden="true">
    {#if focusedLabel}
      <span class="hint-key">{focusedLabel}</span>
      <span>{hovered ? 'Clique para configurar' : 'Selecionado para configurar'}</span>
    {:else}
      <span>Passe o mouse ou selecione um botão</span>
    {/if}
  </div>
</div>

<style>
  .controller-art {
    position: relative;
    width: 100%;
    max-width: 860px;
    padding-bottom: 32px;
    --button-glow: #baf266;
    --button-ink: #20251c;
    --label-bg: #292b2c;
    --label-fg: #f4f5f2;
  }
  .controller {
    display: block;
    width: 100%;
    overflow: visible;
  }
  .artwork {
    pointer-events: none;
    filter: drop-shadow(0 28px 26px #0004);
  }
  .shell-light {
    display: none;
  }
  :global(:root[data-theme='light']) .controller-art {
    --button-glow: #6b9f2b;
    --label-bg: #f0f1ef;
    --label-fg: #30332e;
  }
  :global(:root[data-theme='light']) .shell-light {
    display: block;
  }
  :global(:root[data-theme='light']) .shell-dark {
    display: none;
  }
  @media (prefers-color-scheme: light) {
    :global(:root:not([data-theme='dark'])) .controller-art {
      --button-glow: #6b9f2b;
      --label-bg: #f0f1ef;
      --label-fg: #30332e;
    }
    :global(:root:not([data-theme='dark'])) .shell-light {
      display: block;
    }
    :global(:root:not([data-theme='dark'])) .shell-dark {
      display: none;
    }
  }
  .hit {
    cursor: pointer;
    outline: none;
  }
  .button-outline,
  .halo,
  .rear-cap,
  .rear-leader,
  .rear-anchor {
    transition:
      fill 180ms ease,
      stroke 180ms ease,
      opacity 180ms ease,
      filter 180ms ease,
      transform 180ms ease;
  }
  .button-outline {
    fill: transparent;
    stroke: transparent;
    stroke-width: 5;
    transform-box: fill-box;
    transform-origin: center;
  }
  .halo {
    fill: none;
    stroke: var(--button-glow);
    stroke-width: 16;
    opacity: 0;
    pointer-events: none;
    filter: blur(9px);
  }
  .hit:hover .button-outline,
  .hit:focus-visible .button-outline {
    fill: color-mix(in srgb, var(--button-glow) 16%, transparent);
    stroke: var(--button-glow);
    transform: scale(1.035);
  }
  .hit:hover .halo,
  .hit:focus-visible .halo {
    opacity: 0.45;
  }
  .chosen .button-outline {
    stroke: var(--button-glow);
    fill: color-mix(in srgb, var(--button-glow) 10%, transparent);
    filter: drop-shadow(0 0 7px var(--button-glow));
  }
  .chosen .halo {
    opacity: 0.25;
  }
  .pressed .button-outline {
    fill: color-mix(in srgb, var(--button-glow) 48%, transparent);
    stroke: var(--button-glow);
    transform: scale(0.96);
    filter: drop-shadow(0 0 14px var(--button-glow));
  }
  .pressed .halo {
    opacity: 0.7;
  }
  text {
    text-anchor: middle;
    dominant-baseline: middle;
    font-family: Inter, sans-serif;
    pointer-events: none;
    user-select: none;
  }
  .label-cover {
    fill: var(--label-bg);
    pointer-events: none;
  }
  .face-label {
    fill: var(--label-fg);
    font-size: 60px;
    font-weight: 600;
  }
  .shoulder-label {
    fill: var(--label-fg);
    font-size: 27px;
    font-weight: 750;
  }
  .rear-leader {
    fill: none;
    stroke: var(--muted);
    stroke-width: 3;
    stroke-dasharray: 7 11;
    stroke-linecap: round;
    opacity: 0.55;
    pointer-events: none;
  }
  .rear-anchor {
    fill: var(--surface);
    stroke: var(--muted);
    stroke-width: 3;
    pointer-events: none;
  }
  .rear-cap {
    fill: var(--surface-raised);
    stroke: var(--muted);
    stroke-width: 3;
  }
  .rear-label {
    fill: var(--text);
    font-size: 32px;
    font-weight: 650;
  }
  .rear:hover .rear-cap,
  .rear:focus-within .rear-cap,
  .rear.chosen .rear-cap {
    stroke: var(--button-glow);
    fill: color-mix(in srgb, var(--button-glow) 13%, var(--surface));
    filter: drop-shadow(0 0 13px color-mix(in srgb, var(--button-glow) 40%, transparent));
  }
  .rear:hover .rear-leader,
  .rear:focus-within .rear-leader,
  .rear.chosen .rear-leader,
  .rear.pressed .rear-leader {
    stroke: var(--button-glow);
    opacity: 1;
  }
  .rear.chosen .rear-label,
  .rear:hover .rear-label {
    fill: var(--button-glow);
  }
  .rear.pressed .rear-cap,
  .rear.pressed .rear-anchor {
    fill: var(--button-glow);
    stroke: var(--button-glow);
  }
  .rear.pressed .rear-label {
    fill: var(--button-ink);
  }
  .rear-caption {
    fill: var(--muted);
  }
  .rear-caption path {
    fill: none;
    stroke: var(--muted);
    stroke-width: 3;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .rear-caption text {
    font-size: 22px;
    letter-spacing: 4px;
    font-weight: 500;
  }
  .interaction-hint {
    position: absolute;
    bottom: 12px;
    inset-inline: 0;
    min-height: 24px;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 9px;
    color: var(--muted);
    font-size: 10px;
    pointer-events: none;
  }
  .hint-key {
    min-width: 24px;
    padding: 4px 7px;
    border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--border));
    border-radius: 6px;
    color: var(--accent);
    background: var(--accent-tint);
    font-weight: 600;
  }
  @media (prefers-reduced-motion: reduce) {
    .hit .button-outline,
    .hit .halo,
    .hit .rear-cap,
    .rear .rear-leader,
    .rear .rear-anchor {
      transition: none;
    }
    .hit .button-outline {
      transform: none;
    }
  }
</style>
