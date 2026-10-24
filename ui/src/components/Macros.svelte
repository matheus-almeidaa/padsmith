<script lang="ts">
  import Icon from './Icon.svelte';
  import { request } from '../lib/bridge';
  import {
    makeTarget,
    targetLabel,
    label,
    buttons,
    axes,
    type Macro,
    type MacroStep,
    type MacroTrigger,
    type Profile,
    type Target,
  } from '../lib/types';
  export let profile: Profile;
  export let connected = false,
    device = '',
    recording = false;
  export let edit: (profile: Profile) => void;
  export let fail: (message: string) => void;
  export let notice: (message: string) => void;
  export let learn: (callback: (code: string) => void) => void;
  let selected = '',
    selectedStep = 0,
    dragging = -1,
    newName = '',
    listening = '';
  $: ids = Object.keys(profile.macros ?? {});
  $: if (!ids.includes(selected)) selected = ids[0] ?? '';
  $: sequence = profile.macros?.[selected];
  $: step = sequence?.steps[selectedStep];
  $: triggers = (profile.triggers ?? []).filter((t) => t.macro_id === selected);
  const copy = <T,>(value: T): T => structuredClone(value);
  const pad = (code = 'BTN_SOUTH'): Target => ({ kind: 'gamepad', code });
  const kinds: Record<MacroStep['action'], string> = {
    press: 'Pressionar',
    release: 'Soltar',
    hold: 'Segurar',
    wait: 'Esperar',
    repeat: 'Repetir',
    axis: 'Eixo',
    move: 'Mover mouse',
  };
  const triggerKinds: Record<MacroTrigger['kind'], string> = {
    tap: 'Toque',
    long_press: 'Toque longo',
    double_tap: 'Duplo toque',
    chord: 'Combinação (chord)',
    while_held: 'Repetir enquanto segurado',
    toggle_loop: 'Alternar loop',
  };
  function duration(s: MacroStep): number {
    return 'ms' in s
      ? s.ms
      : s.action === 'repeat'
        ? s.count * s.steps.reduce((n, s) => n + duration(s), 0)
        : 0;
  }
  function defaultStep(action: MacroStep['action']): MacroStep {
    if (action === 'hold') return { action, target: pad(), ms: 100 };
    if (action === 'press' || action === 'release') return { action, target: pad() };
    if (action === 'wait') return { action, ms: 100 };
    if (action === 'repeat')
      return {
        action,
        count: 2,
        steps: [
          { action: 'hold', target: pad(), ms: 80 },
          { action: 'wait', ms: 80 },
        ],
      };
    if (action === 'axis') return { action, code: 'ABS_X', value: 0 };
    return { action, x: 0, y: 0 };
  }
  function update(next: Macro) {
    const p = copy(profile);
    p.macros = { ...p.macros, [selected]: next };
    edit(p);
  }
  function changeStep(next: MacroStep) {
    if (!sequence) return;
    const m = copy(sequence);
    m.steps[selectedStep] = next;
    update(m);
  }
  function add(action: MacroStep['action']) {
    if (!sequence) return;
    const m = copy(sequence);
    m.steps.push(defaultStep(action));
    selectedStep = m.steps.length - 1;
    update(m);
  }
  function removeStep(index: number) {
    if (!sequence || sequence.steps.length <= 1) return;
    const m = copy(sequence);
    m.steps.splice(index, 1);
    selectedStep = Math.max(0, Math.min(selectedStep, m.steps.length - 1));
    update(m);
  }
  function reorder(from: number, to: number) {
    if (!sequence || from < 0 || to < 0 || to >= sequence.steps.length) return;
    const m = copy(sequence);
    const [s] = m.steps.splice(from, 1);
    m.steps.splice(to, 0, s);
    selectedStep = to;
    update(m);
  }
  function create() {
    const id = `macro-${Date.now()}`;
    const p = copy(profile);
    p.macros = {
      ...p.macros,
      [id]: {
        name: newName.trim() || 'Nova sequência',
        steps: [defaultStep('hold'), defaultStep('wait')],
      },
    };
    selected = id;
    selectedStep = 0;
    newName = '';
    edit(p);
  }
  function remove() {
    const p = copy(profile);
    if (p.macros) delete p.macros[selected];
    p.triggers = p.triggers?.filter((t) => t.macro_id !== selected);
    edit(p);
  }
  function addTrigger() {
    const p = copy(profile);
    p.triggers = [
      ...(p.triggers ?? []),
      {
        id: `trigger-${Date.now()}`,
        buttons: ['BTN_WEST'],
        macro_id: selected,
        kind: 'tap',
        threshold_ms: 500,
        window_ms: 250,
        consume: true,
      },
    ];
    edit(p);
  }
  function triggerChange(id: string, change: Partial<MacroTrigger>) {
    const p = copy(profile);
    p.triggers = p.triggers?.map((t) => (t.id === id ? { ...t, ...change } : t));
    edit(p);
  }
  function removeTrigger(id: string) {
    const p = copy(profile);
    p.triggers = p.triggers?.filter((t) => t.id !== id);
    edit(p);
  }
  async function record() {
    try {
      if (!recording) {
        await request({ command: 'record_start', device });
        return;
      }
      const result = await request({ command: 'record_stop', device });
      if (result.event === 'recording') {
        if (result.recording.sequence.steps.every((s) => s.action === 'wait')) {
          notice('Nenhuma ação com saída mapeável foi observada. Verifique o diagnóstico.');
          return;
        }
        const id = `gravacao-${Date.now()}`;
        const p = copy(profile);
        p.macros = { ...p.macros, [id]: result.recording.sequence };
        selected = id;
        selectedStep = 0;
        edit(p);
        if (result.recording.omitted.length)
          notice('Entradas sem saída gravável: ' + result.recording.omitted.join(', '));
        else notice('Gravação concluída. Ajuste os passos e escolha um gatilho.');
      }
    } catch (e) {
      fail(String(e));
    }
  }
  async function play() {
    try {
      await request({ command: 'save_profile', profile });
      await request({ command: 'run_macro', device, macro_id: selected });
      notice('Sequência enviada ao controle virtual.');
    } catch (e) {
      fail(String(e));
    }
  }
  function targetChange(kind: Target['kind'], code?: string) {
    if (!step || !('target' in step)) return;
    changeStep({
      ...step,
      target: makeTarget(kind, code),
    });
  }
</script>

<div class="section-toolbar">
  <div class="inline-create">
    <input
      aria-label="Nome da nova macro"
      placeholder="Nome da nova sequência"
      bind:value={newName}
    /><button class="button primary" on:click={create}
      ><Icon name="plus" size={16} />Nova macro</button
    >
  </div>
  <button
    class="button"
    class:recording
    class:subtle={!recording}
    on:click={record}
    disabled={!connected || !device}
    ><Icon name={recording ? 'pause' : 'record'} size={17} />{recording
      ? 'Encerrar gravação'
      : 'Gravar controle'}</button
  >
</div>
{#if recording}<div class="banner recording-banner" role="status">
    <span class="record-led"></span><span
      ><strong>Gravando eventos físicos.</strong> Pressione os botões e mova os analógicos. As saídas
      virtuais ficam neutralizadas durante a gravação.</span
    >
  </div>{/if}
<div class="macro-workspace">
  <aside class="surface macro-library">
    <span class="overline">SUAS SEQUÊNCIAS · {ids.length}</span>{#each ids as id}<button
        class:chosen={selected === id}
        on:click={() => {
          selected = id;
          selectedStep = 0;
        }}
        ><span class="macro-list-icon"><Icon name="macros" size={17} /></span>
        <div>
          <strong>{profile.macros?.[id].name}</strong><small
            >{profile.macros?.[id].steps.length} passos</small
          >
        </div>
        <Icon name="chevron" size={14} /></button
      >{:else}<p class="help">Crie uma sequência ou grave o controle para começar.</p>{/each}
    <div class="macro-library-tip">
      <Icon name="info" size={17} />
      <p>Tempos explícitos.<br />Gatilhos flexíveis.<br />Saídas sempre liberadas ao cancelar.</p>
    </div>
  </aside>
  <div class="macro-main">
    {#if sequence}<section class="surface macro-editor">
        <div class="row spread">
          <div>
            <span class="overline">EDITOR DE SEQUÊNCIA</span><input
              class="macro-title"
              aria-label="Nome da macro"
              value={sequence.name}
              on:change={(e) => update({ ...sequence, name: e.currentTarget.value })}
            />
          </div>
          <div class="row">
            <button
              class="button subtle"
              on:click={play}
              disabled={!connected || !device || recording}
              ><Icon name="play" size={15} />Executar uma vez</button
            ><button class="icon-button" aria-label="Excluir macro" on:click={remove}
              ><Icon name="trash" size={17} /></button
            >
          </div>
        </div>
        <div class="timeline-ruler">
          <span>0 ms</span><span
            >{sequence.steps.reduce((n, s) => n + duration(s), 0)} ms no total</span
          >
        </div>
        <div class="timeline" role="list" aria-label="Linha do tempo da macro">
          {#each sequence.steps as s, i}<div
              class="timeline-item"
              class:selected={selectedStep === i}
              class:waiting={s.action === 'wait'}
              draggable="true"
              role="listitem"
              on:dragstart={() => (dragging = i)}
              on:dragover={(e) => e.preventDefault()}
              on:drop={(e) => {
                e.preventDefault();
                reorder(dragging, i);
                dragging = -1;
              }}
              style={'--step-width:' + Math.max(110, Math.min(210, 110 + duration(s) / 5)) + 'px'}
            >
              <button class="step-select" on:click={() => (selectedStep = i)}
                ><span class="step-index"
                  ><Icon name="grip" size={13} />{String(i + 1).padStart(2, '0')}</span
                ><strong
                  >{'target' in s && 'code' in s.target
                    ? targetLabel(s.target, profile.layout)
                    : s.action === 'axis'
                      ? s.code
                      : s.action === 'repeat'
                        ? s.count + '×'
                        : s.action === 'move'
                          ? 'Mouse'
                          : 'Pausa'}</strong
                ><small>{kinds[s.action]}{'ms' in s ? ' · ' + s.ms + ' ms' : ''}</small></button
              >
              <div class="step-controls">
                <button
                  aria-label={'Antecipar passo ' + (i + 1)}
                  disabled={i === 0}
                  on:click={() => reorder(i, i - 1)}>←</button
                ><button
                  aria-label={'Adiar passo ' + (i + 1)}
                  disabled={i === sequence.steps.length - 1}
                  on:click={() => reorder(i, i + 1)}>→</button
                ><button
                  aria-label={'Excluir passo ' + (i + 1)}
                  disabled={sequence.steps.length <= 1}
                  on:click={() => removeStep(i)}><Icon name="close" size={12} /></button
                >
              </div>
            </div>{/each}
        </div>
        <div class="add-step-row">
          <span>ADICIONAR PASSO</span>{#each Object.entries(kinds) as [action, name]}<button
              class="button subtle"
              on:click={() => add(action as MacroStep['action'])}
              ><Icon name="plus" size={12} />{name}</button
            >{/each}
        </div>
        {#if step}<div class="step-inspector">
            <div class="row spread">
              <h3>Passo {selectedStep + 1}</h3>
              <span class="tag">{kinds[step.action]}</span>
            </div>
            <div class="step-fields">
              <label
                >Ação<select
                  aria-label="Ação do passo"
                  value={step.action}
                  on:change={(e) =>
                    changeStep(defaultStep(e.currentTarget.value as MacroStep['action']))}
                  >{#each Object.entries(kinds) as [value, name]}<option {value}>{name}</option
                    >{/each}</select
                ></label
              >
              {#if 'target' in step}<label
                  >Dispositivo<select
                    aria-label="Dispositivo do passo"
                    value={step.target.kind}
                    on:change={(e) => targetChange(e.currentTarget.value as Target['kind'])}
                    ><option value="gamepad">Controle virtual</option><option value="trigger"
                      >Gatilho virtual</option
                    ><option value="dpad">D-pad virtual</option><option value="key">Teclado</option
                    ><option value="mouse">Mouse</option></select
                  ></label
                ><label
                  >Saída{#if step.target.kind === 'gamepad'}<select
                      aria-label="Saída do passo"
                      value={step.target.code}
                      on:change={(e) => targetChange('gamepad', e.currentTarget.value)}
                      >{#each buttons as b}<option value={b}
                          >{label(b, profile.layout)} · {b}</option
                        >{/each}</select
                    >{:else if step.target.kind !== 'disabled'}<input
                      aria-label="Código do passo"
                      value={step.target.code +
                        (step.target.kind === 'dpad' ? ':' + step.target.value : '')}
                      on:change={(e) => {
                        if ('target' in step) targetChange(step.target.kind, e.currentTarget.value);
                      }}
                    />{/if}</label
                >{/if}
              {#if 'ms' in step}<label
                  >Duração (ms)<input
                    aria-label="Duração do passo em ms"
                    type="number"
                    min={step.action === 'hold' ? 1 : 0}
                    max="600000"
                    value={step.ms}
                    on:change={(e) => {
                      if ('ms' in step) changeStep({ ...step, ms: +e.currentTarget.value });
                    }}
                  /></label
                >{/if}
              {#if step.action === 'repeat'}<label
                  >Repetições<input
                    aria-label="Contagem da repetição"
                    type="number"
                    min="1"
                    max="1000"
                    value={step.count}
                    on:change={(e) => {
                      if (step.action === 'repeat')
                        changeStep({ ...step, count: +e.currentTarget.value });
                    }}
                  /></label
                >
                <details class="repeat-details">
                  <summary>Editar passos internos (JSON)</summary><textarea
                    aria-label="Passos internos da repetição"
                    value={JSON.stringify(step.steps, null, 2)}
                    on:change={(e) => {
                      try {
                        if (step.action === 'repeat')
                          changeStep({ ...step, steps: JSON.parse(e.currentTarget.value) });
                      } catch (err) {
                        fail(String(err));
                      }
                    }}
                  ></textarea>
                </details>{/if}
              {#if step.action === 'axis'}<label
                  >Eixo<select
                    aria-label="Eixo do passo"
                    value={step.code}
                    on:change={(e) => {
                      if (step.action === 'axis')
                        changeStep({ ...step, code: e.currentTarget.value });
                    }}
                    >{#each axes as code}<option value={code}>{code}</option>{/each}</select
                  ></label
                ><label
                  >Valor normalizado<input
                    type="number"
                    aria-label="Valor do eixo no passo"
                    min="-1"
                    max="1"
                    step="0.01"
                    value={step.value}
                    on:change={(e) => {
                      if (step.action === 'axis')
                        changeStep({ ...step, value: +e.currentTarget.value });
                    }}
                  /></label
                >{/if}
              {#if step.action === 'move'}<label
                  >Horizontal (px)<input
                    type="number"
                    aria-label="Movimento horizontal"
                    min="-10000"
                    max="10000"
                    value={step.x}
                    on:change={(e) => {
                      if (step.action === 'move')
                        changeStep({ ...step, x: +e.currentTarget.value });
                    }}
                  /></label
                ><label
                  >Vertical (px)<input
                    type="number"
                    aria-label="Movimento vertical"
                    min="-10000"
                    max="10000"
                    value={step.y}
                    on:change={(e) => {
                      if (step.action === 'move')
                        changeStep({ ...step, y: +e.currentTarget.value });
                    }}
                  /></label
                >{/if}
            </div>
          </div>{/if}
      </section>
      <section class="surface trigger-editor">
        <div class="row spread">
          <div>
            <span class="overline">ATIVAÇÃO</span>
            <h2>O gesto que inicia tudo.</h2>
          </div>
          <button class="button subtle" on:click={addTrigger}
            ><Icon name="plus" size={15} />Adicionar gatilho</button
          >
        </div>
        {#each triggers as trigger}<div class="trigger-row">
            <div class="trigger-fields">
              <label
                >Quando<select
                  value={trigger.kind}
                  aria-label="Tipo de gatilho"
                  on:change={(e) =>
                    triggerChange(trigger.id, {
                      kind: e.currentTarget.value as MacroTrigger['kind'],
                    })}
                  >{#each Object.entries(triggerKinds) as [value, name]}<option {value}
                      >{name}</option
                    >{/each}</select
                ></label
              ><label
                >Botões físicos<input
                  aria-label="Botões do gatilho"
                  value={trigger.buttons.join(' + ')}
                  on:change={(e) =>
                    triggerChange(trigger.id, {
                      buttons: e.currentTarget.value
                        .split('+')
                        .map((s) => s.trim())
                        .filter(Boolean),
                    })}
                /></label
              ><button
                class="icon-button"
                aria-label="Ouvir botão do gatilho"
                on:click={() => {
                  listening = trigger.id;
                  learn((code) => {
                    triggerChange(trigger.id, { buttons: [code] });
                    listening = '';
                  });
                }}><Icon name="record" size={18} /></button
              >{#if trigger.kind === 'long_press' || trigger.kind === 'tap'}<label
                  >Limiar (ms)<input
                    type="number"
                    min="1"
                    max="60000"
                    aria-label="Limiar do gatilho"
                    value={trigger.threshold_ms}
                    on:change={(e) =>
                      triggerChange(trigger.id, { threshold_ms: +e.currentTarget.value })}
                  /></label
                >{:else if trigger.kind === 'double_tap'}<label
                  >Intervalo (ms)<input
                    type="number"
                    min="1"
                    max="5000"
                    aria-label="Intervalo do duplo toque"
                    value={trigger.window_ms}
                    on:change={(e) =>
                      triggerChange(trigger.id, { window_ms: +e.currentTarget.value })}
                  /></label
                >{/if}<button
                class="icon-button"
                aria-label="Excluir gatilho"
                on:click={() => removeTrigger(trigger.id)}><Icon name="trash" size={16} /></button
              >
            </div>
            <label class="checkbox-row"
              ><input
                type="checkbox"
                checked={trigger.consume}
                on:change={(e) => triggerChange(trigger.id, { consume: e.currentTarget.checked })}
              />Consumir os botões originais</label
            >{#if listening === trigger.id}<p class="help">
                Pressione um botão para identificar seu código.
              </p>{/if}{#if trigger.kind === 'chord'}<p class="help">
                Separe códigos com +. Com consumo ativado, todos os botões participantes ficam
                reservados para o chord.
              </p>{/if}
          </div>{:else}<p class="help">
            Sem gatilhos. Adicione um para executar esta sequência pelo controle.
          </p>{/each}
      </section>
    {:else}<section class="surface empty-macros">
        <span class="macro-illustration"><Icon name="macros" size={52} /></span>
        <h2>Uma sequência. Um gesto.</h2>
        <p class="muted">Grave o que você faz ou construa cada passo na linha do tempo.</p>
        <button class="button primary" on:click={create}
          ><Icon name="plus" size={16} />Criar primeira macro</button
        >
      </section>{/if}
  </div>
</div>

<style>
  .macro-workspace {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 20px;
    align-items: start;
  }
  .macro-library {
    padding: 20px 12px;
    min-height: 390px;
    display: flex;
    flex-direction: column;
  }
  .macro-library > .overline {
    padding-inline: 9px;
    margin-bottom: 18px;
  }
  .macro-library > button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 13px 10px;
    border-radius: 7px;
    text-align: left;
    color: var(--muted);
  }
  .macro-library > button.chosen {
    background: var(--accent-tint);
    color: var(--accent);
  }
  .macro-library > button strong {
    display: block;
    font-size: 11px;
    font-weight: 500;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .macro-library > button small {
    display: block;
    font-size: 9px;
    color: var(--subtle);
    margin-top: 6px;
  }
  .macro-library > button > :global(svg) {
    margin-left: auto;
  }
  .macro-list-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 30px;
    border: 1px solid var(--border);
    border-radius: 6px;
    flex-shrink: 0;
  }
  .macro-library-tip {
    display: flex;
    gap: 10px;
    margin-top: auto;
    padding: 35px 9px 0;
    font-size: 9px;
    color: var(--subtle);
  }
  .macro-library-tip p {
    margin: 0;
  }
  .macro-main {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }
  .macro-editor,
  .trigger-editor {
    padding: 24px;
    min-width: 0;
  }
  .macro-title {
    background: none;
    border: none;
    padding: 0;
    font-size: 20px;
    font-weight: 600;
    min-height: 30px;
    max-width: 290px;
    box-shadow: none;
  }
  .timeline-ruler {
    display: flex;
    justify-content: space-between;
    font: 9px monospace;
    color: var(--subtle);
    padding-top: 30px;
    margin-bottom: 11px;
    border-bottom: 1px solid var(--border);
    padding-bottom: 10px;
  }
  .timeline {
    display: flex;
    gap: 9px;
    overflow-x: auto;
    padding: 5px 2px 17px;
    min-height: 145px;
  }
  .timeline-item {
    flex: 0 0 var(--step-width);
    border: 1px solid var(--border);
    background: var(--accent-tint);
    border-radius: 8px;
    overflow: hidden;
  }
  .timeline-item.selected {
    border-color: var(--accent);
  }
  .timeline-item.waiting {
    background: var(--input);
    border-style: dashed;
  }
  .step-select {
    padding: 12px;
    width: 100%;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 11px;
  }
  .step-index {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--subtle);
    font: 8px monospace;
  }
  .step-select strong {
    color: var(--accent);
    font-size: 17px;
    font-weight: 500;
  }
  .waiting .step-select strong {
    color: var(--muted);
    font-size: 12px;
    padding: 3px 0;
  }
  .step-select small {
    font-size: 9px;
    color: var(--muted);
  }
  .step-controls {
    display: flex;
    justify-content: flex-end;
    border-top: 1px solid var(--border);
    background: var(--input);
  }
  .step-controls button {
    font-size: 11px;
    color: var(--subtle);
    padding: 5px 10px;
  }
  .step-controls button:hover {
    color: var(--text);
    background: var(--surface-raised);
  }
  .add-step-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 7px;
    margin: 5px 0 23px;
  }
  .add-step-row > span {
    font-size: 8px;
    letter-spacing: 1px;
    color: var(--subtle);
    width: 100%;
    margin-bottom: 5px;
  }
  .add-step-row .button {
    font-size: 9px;
    padding: 7px 9px;
    min-height: 29px;
  }
  .step-inspector {
    padding-top: 20px;
    border-top: 1px solid var(--border);
  }
  .step-inspector h3 {
    margin: 0;
    font-size: 12px;
  }
  .step-fields,
  .trigger-fields {
    display: flex;
    flex-wrap: wrap;
    align-items: end;
    gap: 12px;
    margin-top: 15px;
  }
  .step-fields label,
  .trigger-fields label {
    font-size: 9px;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 7px;
    flex: 1;
    min-width: 100px;
  }
  .step-fields input,
  .step-fields select,
  .trigger-fields input,
  .trigger-fields select {
    font-size: 11px;
    width: 100%;
    min-height: 37px;
  }
  .trigger-fields label:nth-child(2) {
    flex: 2;
    min-width: 170px;
  }
  .trigger-editor h2 {
    font-size: 16px;
    margin: 0;
  }
  .trigger-row {
    margin-top: 20px;
    border-top: 1px solid var(--border);
    padding-top: 5px;
  }
  .trigger-row .checkbox-row {
    font-size: 10px;
    margin-top: 14px;
  }
  .trigger-row .help {
    margin-bottom: 0;
  }
  .recording {
    background: #ff7f7618 !important;
    color: #ff9b94 !important;
    border-color: #ff9b9450 !important;
  }
  .recording-banner {
    border-color: #ff9b9430;
    color: var(--text);
  }
  .record-led {
    background: #ff7f76;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    box-shadow: 0 0 0 4px #ff7f7618;
    flex-shrink: 0;
  }
  .repeat-details {
    flex-basis: 100%;
    font-size: 10px;
    color: var(--muted);
  }
  textarea {
    background: var(--input);
    border: 1px solid var(--border);
    color: var(--text);
    width: 100%;
    min-height: 140px;
    font: 11px monospace;
    padding: 12px;
    border-radius: 7px;
    margin-top: 10px;
  }
  summary {
    cursor: pointer;
  }
  .macro-editor > .row {
    flex-wrap: wrap;
  }
  .macro-editor > .row > .row {
    gap: 7px;
  }
  .macro-library > .help {
    padding: 10px;
  }
  @media (max-width: 1200px) {
    .macro-workspace {
      grid-template-columns: 170px minmax(0, 1fr);
      gap: 14px;
    }
    .macro-editor,
    .trigger-editor {
      padding: 20px;
    }
    .macro-library > button strong {
      max-width: 90px;
    }
    .macro-title {
      max-width: 220px;
    }
    .macro-editor > .row > .row .button {
      font-size: 9px;
    }
  }
  @media (max-width: 900px) {
    .macro-workspace {
      grid-template-columns: 1fr;
    }
    .macro-library {
      min-height: auto;
      flex-direction: row;
      overflow: auto;
      padding: 10px;
    }
    .macro-library > .overline,
    .macro-library-tip {
      display: none;
    }
    .macro-library > button {
      min-width: 170px;
    }
    .macro-library > button strong {
      max-width: 110px;
    }
    .trigger-fields {
      gap: 9px;
    }
  }
</style>
