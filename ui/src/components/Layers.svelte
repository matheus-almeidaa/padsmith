<script lang="ts">
  import Icon from './Icon.svelte';
  import type { Emergency, Layer, Profile, ProfileShortcut } from '../lib/types';
  export let profile: Profile;
  export let profiles: Profile[] = [];
  export let edit: (profile: Profile) => void;
  export let learn: (callback: (code: string) => void) => void;
  let name = '',
    listening = '';
  $: emergency = profile.emergency ?? {
    enabled: true,
    buttons: ['BTN_MODE', 'BTN_SELECT'],
    hold_ms: 3000,
  };
  const clone = <T,>(v: T): T => structuredClone(v);
  const codes = (text: string) =>
    text
      .split('+')
      .map((s) => s.trim())
      .filter(Boolean);
  function add() {
    const p = clone(profile);
    const id = `camada-${Date.now()}`;
    p.layers = {
      ...p.layers,
      [id]: {
        name: name.trim() || 'Nova camada',
        activation: ['BTN_THUMBL'],
        priority: 0,
        buttons: {},
        axes: {},
      },
    };
    name = '';
    edit(p);
  }
  function change(id: string, changes: Partial<Layer>) {
    const p = clone(profile);
    if (p.layers?.[id]) p.layers[id] = { ...p.layers[id], ...changes };
    edit(p);
  }
  function remove(id: string) {
    const p = clone(profile);
    if (p.layers) delete p.layers[id];
    edit(p);
  }
  function emergencyChange(changes: Partial<Emergency>) {
    edit({ ...clone(profile), emergency: { ...emergency, ...changes } });
  }
  function shortcutChange(index: number, changes: Partial<ProfileShortcut>) {
    const p = clone(profile);
    if (p.shortcuts) p.shortcuts[index] = { ...p.shortcuts[index], ...changes };
    edit(p);
  }
</script>

<section class="surface layer-settings">
  <div class="row spread">
    <div>
      <span class="overline">CAMADAS</span>
      <h2>Um controle. Mais possibilidades.</h2>
      <p class="muted">
        Segure um botão para acessar outro mapa. Edite o mapa na tela de Remapeamento ou Analógicos.
      </p>
    </div>
    <Icon name="profiles" />
  </div>
  <div class="inline-create">
    <input aria-label="Nome da nova camada" placeholder="Nome da camada" bind:value={name} /><button
      class="button subtle"
      on:click={add}><Icon name="plus" size={15} />Criar camada</button
    >
  </div>
  {#each Object.entries(profile.layers ?? {}) as [id, layer]}<div class="layer-row">
      <div class="layer-fields">
        <label
          >Nome<input
            aria-label="Nome da camada"
            value={layer.name}
            on:change={(e) => change(id, { name: e.currentTarget.value })}
          /></label
        ><label
          >Segurar botões<input
            aria-label="Ativação da camada"
            value={layer.activation.join(' + ')}
            on:change={(e) => change(id, { activation: codes(e.currentTarget.value) })}
          /></label
        ><button
          class="icon-button"
          aria-label="Ouvir ativador da camada"
          on:click={() => {
            listening = id;
            learn((code) => {
              change(id, { activation: [code] });
              listening = '';
            });
          }}><Icon name="record" size={18} /></button
        ><label class="priority-input"
          >Prioridade<input
            aria-label="Prioridade da camada"
            type="number"
            value={layer.priority}
            on:change={(e) => change(id, { priority: +e.currentTarget.value })}
          /></label
        ><button class="icon-button" aria-label="Excluir camada" on:click={() => remove(id)}
          ><Icon name="trash" size={17} /></button
        >
      </div>
      <p class="help">
        {listening === id
          ? 'Aguardando evento de botão em qualquer nó…'
          : `${Object.keys(layer.buttons).length} botões e ${Object.keys(layer.axes).length} eixos personalizados. Ativadores são consumidos pelo Padsmith.`}
      </p>
    </div>{/each}
  <p class="help">
    Para L4/R4/M, identifique primeiro um evento real. Camadas de maior prioridade prevalecem;
    empates usam a ordem do ID.
  </p>
</section>
<section class="surface layer-settings">
  <div>
    <span class="overline">EMERGÊNCIA</span>
    <h2>Uma saída, sempre à mão.</h2>
    <p class="muted">O atalho continua disponível em pausa e durante a gravação.</p>
  </div>
  <label class="checkbox-row"
    ><input
      type="checkbox"
      checked={emergency.enabled}
      on:change={(e) => emergencyChange({ enabled: e.currentTarget.checked })}
    />Ativar atalho para pausar e retomar</label
  >
  <div class="layer-fields">
    <label
      >Combinação física<input
        aria-label="Combinação de emergência"
        value={emergency.buttons.join(' + ')}
        on:change={(e) => emergencyChange({ buttons: codes(e.currentTarget.value) })}
      /></label
    ><label
      >Segurar por (ms)<input
        type="number"
        min="1"
        max="60000"
        aria-label="Tempo de emergência"
        value={emergency.hold_ms}
        on:change={(e) => emergencyChange({ hold_ms: +e.currentTarget.value })}
      /></label
    >
  </div>
  <p class="help">
    Padrão: Home + Select por 3 segundos. Antes de completar o tempo, os botões mantêm suas ações
    normais.
  </p>
</section>
<section class="surface layer-settings">
  <div class="row spread">
    <div>
      <span class="overline">PERFIS PELO CONTROLE</span>
      <h2>Troque sem sair do jogo.</h2>
    </div>
    <button
      class="button subtle"
      on:click={() =>
        edit({
          ...clone(profile),
          shortcuts: [
            ...(profile.shortcuts ?? []),
            { buttons: ['BTN_TL', 'BTN_START'], profile: '$next', hold_ms: 700 },
          ],
        })}><Icon name="plus" size={15} />Adicionar</button
    >
  </div>
  {#each profile.shortcuts ?? [] as shortcut, index}<div class="layer-row">
      <div class="layer-fields">
        <label
          >Combinação<input
            aria-label="Combinação de troca de perfil"
            value={shortcut.buttons.join(' + ')}
            on:change={(e) => shortcutChange(index, { buttons: codes(e.currentTarget.value) })}
          /></label
        ><label
          >Destino<select
            aria-label="Perfil do atalho"
            value={shortcut.profile}
            on:change={(e) => shortcutChange(index, { profile: e.currentTarget.value })}
            ><option value="$next">Próximo perfil</option>{#each profiles as p}<option value={p.id}
                >{p.name}</option
              >{/each}</select
          ></label
        ><label class="priority-input"
          >Tempo (ms)<input
            aria-label="Tempo do atalho de perfil"
            type="number"
            min="1"
            max="60000"
            value={shortcut.hold_ms}
            on:change={(e) => shortcutChange(index, { hold_ms: +e.currentTarget.value })}
          /></label
        ><button
          class="icon-button"
          aria-label="Excluir atalho de perfil"
          on:click={() =>
            edit({
              ...clone(profile),
              shortcuts: profile.shortcuts?.filter((_, i) => i !== index),
            })}><Icon name="trash" size={16} /></button
        >
      </div>
    </div>{/each}
  <p class="help">
    Os botões de atalhos ficam reservados. Configure o atalho nos perfis entre os quais você quer
    alternar.
  </p>
</section>

<style>
  .layer-settings {
    padding: 27px;
    margin-bottom: 20px;
  }
  .layer-settings h2 {
    font-size: 19px;
  }
  .layer-settings > .row > :global(svg) {
    color: var(--accent);
  }
  .layer-settings .muted {
    font-size: 11px;
    max-width: 510px;
  }
  .layer-fields {
    display: flex;
    align-items: end;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 18px;
  }
  .layer-fields label {
    font-size: 10px;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 7px;
    flex: 1;
    min-width: 150px;
  }
  .layer-fields .priority-input {
    max-width: 100px;
    min-width: 80px;
    flex: 0.4;
  }
  .layer-fields input,
  .layer-fields select {
    width: 100%;
    font-size: 11px;
  }
  .layer-row {
    border-top: 1px solid var(--border);
    margin-top: 20px;
  }
  .layer-row .help {
    margin-bottom: 0;
  }
  .layer-settings .inline-create {
    margin-top: 22px;
  }
  .layer-settings > .help:last-child {
    margin-bottom: 0;
  }
</style>
