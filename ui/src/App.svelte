<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './components/Icon.svelte';
  import ControllerArt from './components/Controller.svelte';
  import Macros from './components/Macros.svelte';
  import Layers from './components/Layers.svelte';
  import { t } from './lib/i18n';
  import { invoke, native, request, subscribe } from './lib/bridge';
  import {
    axes,
    buttons,
    defaultAxis,
    defaultProfile,
    defaultTarget,
    makeTarget,
    targetFromCode,
    targetLabel,
    label,
    type AxisBinding,
    type Binding,
    type Check,
    type Message,
    type Profile,
    type RawEvent,
    type Status,
    type Target,
  } from './lib/types';

  type Page = keyof typeof t.nav;
  const navigation: Page[] = ['profiles', 'remap', 'macros', 'axes', 'diagnostics', 'settings'];
  let page: Page = 'remap';
  let status: Status = { controllers: [], profiles: [], version: '0.1.0' };
  let connected = false,
    selectedDevice = '',
    selected = 'BTN_WEST';
  let profile = defaultProfile(),
    history: Profile[] = [],
    dirty = false,
    saving = false;
  let error = '',
    toast = '',
    listening = false,
    filter = '',
    freeze = false;
  function emptyButtons() {
    return new Set<string>();
  }
  let raw: RawEvent[] = [];
  let active = emptyButtons();
  let seen = emptyButtons();
  let checks: Check[] = [],
    checking = false,
    profileName = '';
  let theme = localStorage.getItem('padsmith-theme') ?? 'system';
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  let listenTimer: ReturnType<typeof setTimeout> | undefined;
  let listenRoles = new Set<string>();
  let listenDropped = 0;
  let streamBuffer: RawEvent[] = [];
  let liveAxes: Record<string, number> = {};
  let learnCallback: ((code: string) => void) | null = null;
  let hardwareNotice = '';
  let editingLayer = '',
    editRevision = 0;
  $: if (editingLayer && !profile.layers?.[editingLayer]) editingLayer = '';
  $: currentMap = editingLayer ? (profile.layers?.[editingLayer] ?? profile) : profile;
  $: controller = status.controllers.find((c) => c.device.id === selectedDevice);
  $: if (page !== 'remap' || !connected || !controller || controller.error) stopListening();
  $: mappedCount = Object.keys(currentMap.buttons).length;
  $: rear = selected.startsWith('rear_');
  $: selectedLabel = label(
    Object.entries(profile.aliases ?? {}).find(([, source]) => source === selected)?.[0] ??
      selected,
    profile.layout,
  );
  $: binding =
    currentMap.buttons[selected] ??
    (editingLayer ? profile.buttons[selected] : undefined) ??
    ({
      mode: 'hold',
      output: defaultTarget(selected),
    } as Binding);
  $: outputKind = binding.output?.kind ?? 'gamepad';
  $: outputCode =
    binding.output && 'code' in binding.output
      ? binding.output.code + (binding.output.kind === 'dpad' ? ':' + binding.output.value : '')
      : '';
  $: filteredRaw = raw.filter(
    (e) =>
      !filter ||
      `${e.label} ${e.node} ${e.role} ${e.event_type} ${e.code}`
        .toLowerCase()
        .includes(filter.toLowerCase()),
  );
  $: document.documentElement.dataset.theme = theme;
  function notify(message: string) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 4500);
  }
  function stopListening() {
    clearTimeout(listenTimer);
    listenTimer = undefined;
    listening = false;
  }
  function toggleListening() {
    if (listening) {
      stopListening();
      return;
    }
    if (!connected || !controller || controller.error) return;
    learnCallback = null;
    hardwareNotice = '';
    listenRoles = new Set();
    listenDropped = 0;
    listening = true;
    const isRear = selected.startsWith('rear_');
    const name = label(selected, profile.layout);
    listenTimer = setTimeout(() => {
      stopListening();
      if (listenDropped) {
        hardwareNotice = `A captura perdeu ${listenDropped} eventos e não identificou um pressionamento para ${name}. Repita o teste; esse resultado não permite concluir se o botão envia um evento.`;
        return;
      }
      const traffic = listenRoles.size
        ? `Foram recebidos dados de ${[...listenRoles].join(', ')}, mas nenhum pressionamento de botão.`
        : 'Nenhum dado de entrada foi recebido. Confira a conexão e a captura em Diagnóstico.';
      hardwareNotice = `Nenhum botão identificado para ${name} em 15 segundos. ${traffic} ${
        isRear
          ? 'Um traseiro sem comando no software oficial pode não enviar evento neste modo. O Padsmith remapeia eventos evdev; configurar o botão no firmware exige suporte específico ao protocolo do fabricante. Compare com um botão comum no diagnóstico.'
          : 'Pressione e solte um botão comum e compare os eventos no diagnóstico.'
      }`;
    }, 15000);
  }
  const clone = <T,>(value: T): T => structuredClone(value);
  function receive(message: Message) {
    if (message.event === 'status') {
      connected = true;
      status = message.status;
      if (!status.controllers.some((c) => c.device.id === selectedDevice)) {
        selectedDevice = status.controllers[0]?.device.id ?? '';
        active = new Set();
        liveAxes = {};
        seen = new Set();
        raw = [];
        streamBuffer = [];
        stopListening();
        learnCallback = null;
        hardwareNotice = '';
      }
      if (!dirty && !saving) {
        const activeProfile = status.controllers.find(
          (c) => c.device.id === selectedDevice,
        )?.profile;
        profile = clone(
          status.profiles.find((p) => p.id === (activeProfile ?? profile.id)) ??
            status.profiles[0] ??
            defaultProfile(),
        );
      }
    } else if (message.event === 'raw' && message.device === selectedDevice) {
      if (!freeze) streamBuffer.push(message);
      if (listening && message.event_type !== 0) listenRoles.add(message.role);
      if (message.event_type === 3 && message.role === 'gamepad') {
        const range = controller?.device.nodes.find((n) => n.path === message.node)?.axes?.[
          message.label
        ];
        if (range && range.maximum > range.minimum) {
          const unit = Math.max(
            0,
            Math.min(1, (message.value - range.minimum) / (range.maximum - range.minimum)),
          );
          liveAxes = {
            ...liveAxes,
            [message.label]: ['ABS_Z', 'ABS_RZ'].includes(message.label) ? unit : unit * 2 - 1,
          };
        }
      }
      if (message.event_type === 1) {
        const code =
          message.role === 'gamepad' ? message.label : `${message.role}:${message.label}`;
        if (message.value === 1) {
          if (learnCallback) {
            const callback = learnCallback;
            learnCallback = null;
            callback(code);
          }
          seen = new Set([...seen, code]);
          active = new Set([...active, code]);
          if (listening) {
            if (selected.startsWith('rear_')) {
              const shared =
                defaultTarget(code).kind !== 'disabled' ||
                Object.entries(profile.aliases ?? {}).some(
                  ([rear, source]) => rear !== selected && source === code,
                );
              if (shared)
                hardwareNotice = `Foi observado ${code}, um código padrão ou já associado. Esta captura não comprova um traseiro independente. Se outro botão enviar o mesmo código, o remapeamento afetará ambos; compare no diagnóstico.`;
              else {
                const next = clone(profile);
                next.aliases = { ...next.aliases, [selected]: code };
                edit(next);
                hardwareNotice = `Associação ao evento ${code} registrada. Compare com os demais botões no diagnóstico para confirmar que o código não é compartilhado.`;
              }
            }
            selected = code;
            stopListening();
            notify(`Evento identificado: ${code}`);
          }
        } else if (message.value === 0) {
          active = new Set([...active].filter((c) => c !== code));
        }
      }
    } else if (message.event === 'notice' || message.event === 'error') notify(message.message);
    else if (message.event === 'dropped') {
      if (listening) listenDropped += message.count;
      notify(`O monitor perdeu ${message.count} eventos. O processamento de saída é independente.`);
    }
  }
  onMount(() => {
    let disposed = false,
      cleanup = () => {};
    subscribe(receive, () => {
      connected = false;
      active = new Set();
      liveAxes = {};
      stopListening();
    })
      .then((fn) => {
        if (disposed) fn();
        else cleanup = fn;
      })
      .catch((e) => (error = String(e)));
    request({ command: 'status' })
      .then(receive)
      .catch(() => {});
    const render = setInterval(() => {
      if (streamBuffer.length) {
        raw = [...streamBuffer.reverse(), ...raw].slice(0, 250);
        streamBuffer = [];
      }
    }, 50);
    return () => {
      disposed = true;
      cleanup();
      clearInterval(render);
      clearTimeout(saveTimer);
      clearTimeout(toastTimer);
      stopListening();
    };
  });
  async function persist(next: Profile, revision: number) {
    saving = true;
    try {
      await request({ command: 'save_profile', profile: next });
      if (revision === editRevision) {
        dirty = false;
        error = '';
      }
    } catch (e) {
      if (revision === editRevision) error = String(e);
    } finally {
      saving = false;
    }
  }
  function edit(next: Profile, remember = true) {
    if (remember) history = [...history.slice(-29), clone(profile)];
    profile = next;
    editRevision += 1;
    const revision = editRevision;
    dirty = true;
    clearTimeout(saveTimer);
    if (connected) saveTimer = setTimeout(() => persist(clone(profile), revision), 250);
    else error = 'Alteração não aplicada: daemon indisponível.';
  }
  function changeBinding(change: Partial<Binding>) {
    const next = clone(profile);
    const map = editingLayer ? (next.layers?.[editingLayer] ?? next) : next;
    map.buttons[selected] = { ...binding, ...change };
    edit(next);
  }
  function changeOutput(kind: Target['kind'], code?: string) {
    const output = makeTarget(kind, code);
    changeBinding({ output });
  }
  function resetBinding() {
    const next = clone(profile);
    const map = editingLayer ? (next.layers?.[editingLayer] ?? next) : next;
    delete map.buttons[selected];
    edit(next);
  }
  function undo() {
    const previous = history.at(-1);
    if (previous) {
      history = history.slice(0, -1);
      edit(previous, false);
    }
  }
  async function activate(id: string) {
    const chosen = status.profiles.find((p) => p.id === id);
    if (!chosen) return;
    if (dirty) {
      error = 'Aguarde aplicar ou desfaça a alteração pendente antes de trocar de perfil.';
      return;
    }
    stopListening();
    try {
      await request({ command: 'apply', device: selectedDevice || null, profile: id });
      profile = clone(chosen);
      history = [];
      notify(`Perfil ${chosen.name} ativado`);
    } catch (e) {
      error = String(e);
    }
  }
  async function pause() {
    try {
      await request({
        command: 'pause',
        device: selectedDevice || null,
        paused: !controller?.paused,
      });
    } catch (e) {
      error = String(e);
    }
  }
  function choose(code: string) {
    stopListening();
    hardwareNotice = '';
    if (code.startsWith('rear_') && profile.aliases?.[code]) {
      code = profile.aliases[code];
      if (defaultTarget(code).kind !== 'disabled')
        hardwareNotice =
          'Esta associação usa o mesmo código de um botão padrão. O remapeamento se aplica a todas as entradas que enviam esse código.';
    }
    if (code === 'trigger_left' || code === 'trigger_right') {
      const button = code === 'trigger_left' ? 'BTN_TL2' : 'BTN_TR2';
      const axis = code === 'trigger_left' ? 'ABS_Z' : 'ABS_RZ';
      if (
        controller?.device.nodes.some(
          (n) => n.role === 'gamepad' && n.keys?.includes(button) && !n.axes?.[axis],
        )
      )
        selected = button;
      else page = 'axes';
      return;
    }
    if (code === 'dpad') {
      page = 'axes';
      return;
    }
    selected = code;
  }
  async function importProfile() {
    try {
      const imported = await invoke<Profile | null>('import_profile');
      if (imported) {
        profile = imported;
        notify('Perfil importado');
      }
    } catch (e) {
      error = String(e);
    }
  }
  async function exportProfile() {
    try {
      if (await invoke<boolean>('export_profile', { profile })) notify('Perfil exportado');
    } catch (e) {
      error = String(e);
    }
  }
  async function createProfile() {
    if (!profileName.trim()) return;
    const id =
      profileName
        .normalize('NFD')
        .replace(/[\u0300-\u036f]/g, '')
        .toLowerCase()
        .replace(/[^a-z0-9_-]/g, '-')
        .slice(0, 60) || `perfil-${Date.now()}`;
    if (status.profiles.some((p) => p.id === id)) {
      error = 'Já existe um perfil com esse ID.';
      return;
    }
    const next = { ...defaultProfile(), id, name: profileName.trim() };
    try {
      await request({ command: 'save_profile', profile: next });
      profileName = '';
      notify('Perfil criado');
    } catch (e) {
      error = String(e);
    }
  }
  function axisChange(code: string, change: Partial<AxisBinding>) {
    const next = clone(profile);
    const map = editingLayer ? (next.layers?.[editingLayer] ?? next) : next;
    map.axes[code] = { ...defaultAxis(), ...map.axes[code], ...change };
    edit(next);
  }
  async function runDoctor() {
    checking = true;
    try {
      checks = await invoke<Check[]>('doctor');
    } catch (e) {
      error = String(e);
    } finally {
      checking = false;
    }
  }
  function updateTheme(value: string) {
    theme = value;
    localStorage.setItem('padsmith-theme', value);
  }
  function curvePath(binding: AxisBinding) {
    return Array.from({ length: 41 }, (_, i) => {
      const x = i / 40;
      const v =
        x <= binding.deadzone
          ? 0
          : Math.pow((x - binding.deadzone) / (1 - binding.deadzone), binding.curve);
      return `${i ? 'L' : 'M'}${20 + x * 220},${110 - v * 90}`;
    }).join(' ');
  }
</script>

<svelte:head><title>{t.nav[page]} · Padsmith</title></svelte:head>
<div class="app-shell">
  <aside class="sidebar">
    <a class="brand" href="#remap" on:click={() => (page = 'remap')} aria-label="Padsmith, início"
      ><img src="/padsmith.svg" alt="" /><span>padsmith<span class="brand-dot">.</span></span></a
    >
    <div class="workspace-label">{t.workspace}</div>
    <nav aria-label="Navegação principal">
      {#each navigation as item}<button
          aria-label={t.nav[item]}
          aria-current={page === item ? 'page' : undefined}
          class:current={page === item}
          on:click={() => (page = item)}
          ><Icon name={item} /><span>{t.nav[item]}</span>{#if item === 'macros'}<span
              class="nav-count">↗</span
            >{/if}</button
        >{/each}
    </nav>
    <div class="sidebar-note">
      <span class="mini-icon"><Icon name="bolt" size={18} /></span><strong>Feito para jogar.</strong
      >
      <p>Seu perfil continua ativo<br />com a janela fechada.</p>
    </div>
    <div class="daemon-status">
      <span class:online={connected} class="status-dot"></span>
      <div>
        <strong>{connected ? t.online : t.offline}</strong><small>Padsmith {status.version}</small>
      </div>
    </div>
  </aside>
  <div class="main-shell">
    <header class="topbar">
      <div class="breadcrumb">
        Meu espaço <Icon name="chevron" size={13} /><span>{t.nav[page]}</span>
      </div>
      <div class="device-picker">
        <Icon name="pad" />{#if status.controllers.length}<select
            aria-label="Controle selecionado"
            value={selectedDevice}
            on:change={(e) => {
              if (dirty) {
                error =
                  'Aguarde aplicar ou desfaça a alteração pendente antes de trocar de controle.';
                e.currentTarget.value = selectedDevice;
                return;
              }
              selectedDevice = e.currentTarget.value;
              active = new Set();
              liveAxes = {};
              seen = new Set();
              raw = [];
              streamBuffer = [];
              hardwareNotice = '';
              stopListening();
              learnCallback = null;
              dirty = false;
              history = [];
              const nextController = status.controllers.find((c) => c.device.id === selectedDevice);
              const p = status.profiles.find((p) => p.id === nextController?.profile);
              if (p) profile = clone(p);
            }}
            >{#each status.controllers as c}<option value={c.device.id}>{c.device.name}</option
              >{/each}</select
          >{:else}<span>Nenhum controle conectado</span>{/if}<span
          class:online={!!controller && !controller.error}
          class="status-dot"
        ></span>
      </div>
    </header>
    <main>
      <div class="page-heading">
        <div>
          <p class="eyebrow">
            {page === 'remap' ? t.eyebrow : 'PADSMITH / ' + t.nav[page].toUpperCase()}
          </p>
          <h1>{page === 'remap' ? t.title : t.nav[page]}</h1>
          <p class="subtitle">
            {page === 'remap'
              ? t.subtitle
              : page === 'diagnostics'
                ? 'Cada evento, sem adivinhação. Conheça o que seu controle realmente envia.'
                : page === 'profiles'
                  ? 'Um jeito de jogar para cada universo.'
                  : page === 'axes'
                    ? 'Precisão que acompanha o seu movimento.'
                    : page === 'macros'
                      ? 'Transforme uma sequência em um único gesto.'
                      : 'Os detalhes deixam tudo com a sua cara.'}
          </p>
        </div>
        <div class="heading-actions">
          <button
            class="button subtle icon-button"
            title={t.undo}
            aria-label={t.undo}
            disabled={!history.length}
            on:click={undo}><Icon name="undo" /></button
          ><span class="live-label" class:pending={dirty || !connected}
            ><span class="status-dot" class:online={connected && !dirty}></span>{saving
              ? 'Aplicando…'
              : dirty
                ? 'Não aplicado'
                : connected
                  ? t.live
                  : 'Aguardando daemon'}</span
          >
        </div>
      </div>
      {#if error}<div class="banner error" role="alert">
          <Icon name="info" /><span>{error}</span><button
            class="icon-button"
            aria-label="Fechar aviso"
            on:click={() => (error = '')}><Icon name="close" size={16} /></button
          >
        </div>{/if}
      {#if controller?.error}<div class="banner warning">
          <Icon name="info" /><span
            ><strong>Controle não capturado.</strong>
            {controller.error} Confira as permissões de todos os nós em Diagnóstico.</span
          >
        </div>{/if}
      {#if hardwareNotice}<div class="banner warning" role="status">
          <Icon name="info" /><span>{hardwareNotice}</span><button
            class="icon-button"
            aria-label="Fechar aviso de hardware"
            on:click={() => (hardwareNotice = '')}><Icon name="close" size={15} /></button
          >
        </div>{/if}

      {#if (page === 'remap' || page === 'axes') && Object.keys(profile.layers ?? {}).length}<div
          class="map-selector-bar"
        >
          <label for="editing-map">MAPA EM EDIÇÃO</label><select
            id="editing-map"
            aria-label="Mapa em edição"
            bind:value={editingLayer}
            ><option value="">Mapa base</option
            >{#each Object.entries(profile.layers ?? {}) as [id, layer]}<option value={id}
                >{layer.name}</option
              >{/each}</select
          ><span class="help"
            >{editingLayer
              ? 'A camada só atua enquanto seu ativador estiver pressionado.'
              : 'As camadas herdam os destinos não personalizados do mapa base.'}</span
          >{#each controller?.layers ?? [] as id}<span class="tag green"
              >{profile.layers?.[id]?.name ?? id} ATIVA</span
            >{/each}
        </div>{/if}
      {#if page === 'remap'}
        <div class="profile-toolbar">
          <div class="profile-select">
            <span class="profile-symbol"><Icon name="profiles" size={20} /></span>
            <div>
              <span class="overline"
                >{controller && controller.profile !== profile.id
                  ? 'EDITANDO PERFIL'
                  : controller?.automatic
                    ? 'PERFIL AUTOMÁTICO'
                    : 'PERFIL ATIVO'}</span
              ><select
                aria-label="Perfil ativo"
                value={profile.id}
                on:change={(e) => activate(e.currentTarget.value)}
                >{#if !status.profiles.length}<option value={profile.id}>{profile.name}</option
                  >{/if}{#each status.profiles as p}<option value={p.id}>{p.name}</option
                  >{/each}</select
              >
            </div>
            <Icon name="down" size={16} />
          </div>
          <div class="toolbar-separator"></div>
          <span class="toolbar-meta"><strong>{mappedCount}</strong> associações personalizadas</span
          >
          <div class="toolbar-end">
            {#if controller?.device.battery != null}<span class="battery"
                ><Icon name="battery" size={18} />{controller.device.battery}%</span
              >{/if}<button
              class="button subtle"
              on:click={pause}
              disabled={!controller || !!controller.error}
              ><Icon name={controller?.paused ? 'play' : 'pause'} size={16} />{controller?.paused
                ? t.resume
                : t.pause}</button
            >
          </div>
        </div>
        <div class="mapping-layout">
          <section class="controller-card">
            <div class="card-top">
              <div class="section-label">
                <span class="status-dot" class:online={!!controller && !controller.error}
                ></span>{controller && !controller.error
                  ? 'CONTROLE CAPTURADO'
                  : 'VISUALIZAÇÃO DO CONTROLE'}
              </div>
              <span class="outline-tag"
                >{profile.layout === 'xbox'
                  ? 'Xbox'
                  : profile.layout === 'playstation'
                    ? 'PlayStation'
                    : 'Nintendo'}</span
              >
            </div>
            <div class="controller-stage">
              <div class="controller-glow"></div>
              <ControllerArt
                {selected}
                {active}
                axes={liveAxes}
                aliases={profile.aliases ?? {}}
                layout={profile.layout}
                {choose}
              />
            </div>
            <div class="controller-caption">
              <span><span class="legend-dot"></span>Selecionado</span><span
                ><span class="legend-dot filled"></span>Pressionado ao vivo</span
              ><span class="caption-end"
                ><Icon name="info" size={14} />Botões traseiros dependem dos eventos do dispositivo</span
              >
            </div>
            <div class="controller-footer">
              <Icon name="pad" size={24} />
              <div>
                <strong
                  >{controller
                    ? 'Controle virtual Xbox 360'
                    : 'Pronto para o seu próximo jogo'}</strong
                ><small
                  >{controller
                    ? '045e:028e · evdev → uinput'
                    : 'Conecte um controle e inicie o daemon para ver os eventos ao vivo.'}</small
                >
              </div>
              <span class="outline-tag">LINUX</span>
            </div>
          </section>
          <section class="binding-panel">
            <div class="panel-head">
              <span class="overline">PERSONALIZAR BOTÃO</span><span
                class="kbd-symbol"
                title={selectedLabel}>{selectedLabel.length > 4 ? '•' : selectedLabel}</span
              >
            </div>
            <h2>
              {rear ? 'Vamos identificar ' + label(selected) : 'Botão ' + selectedLabel}
            </h2>
            <p class="muted">
              {rear
                ? 'Primeiro identifique o evento recebido; depois escolha uma saída de controle, teclado ou mouse.'
                : 'Escolha o que acontece quando você pressiona este botão.'}
            </p>
            {#if rear}<div class="info-box">
                <Icon name="info" size={18} />
                <p>
                  Um traseiro sem comando no software oficial pode não enviar evento ao Linux. O
                  Padsmith remapeia eventos recebidos; ele não grava comandos no firmware do
                  controle. A escuta verifica o modo atual por até 15 segundos.
                </p>
              </div>
              <button
                class="button primary full"
                on:click={toggleListening}
                aria-pressed={listening}
                disabled={!connected || !controller || !!controller.error}
                ><Icon name="record" size={16} />{listening
                  ? 'Cancelar escuta · até 15 s'
                  : 'Ouvir próximo botão'}</button
              ><button class="text-button" on:click={() => (page = 'diagnostics')}
                >Abrir diagnóstico completo <Icon name="chevron" size={14} /></button
              >
            {:else}<label class="field-label" for="output-kind">ENVIAR COMO</label><select
                id="output-kind"
                value={outputKind}
                on:change={(e) => changeOutput(e.currentTarget.value as Target['kind'])}
                ><option value="gamepad">Botão do controle virtual</option><option value="trigger"
                  >Gatilho virtual LT/RT</option
                ><option value="dpad">Direção do d-pad virtual</option><option value="key"
                  >Tecla do teclado</option
                ><option value="mouse">Botão do mouse</option><option value="disabled"
                  >Nada · desativado</option
                ></select
              >
              {#if outputKind !== 'disabled'}<label class="field-label" for="output-code"
                  >SAÍDA</label
                >{#if outputKind === 'gamepad'}<select
                    id="output-code"
                    value={outputCode}
                    on:change={(e) => changeOutput('gamepad', e.currentTarget.value)}
                    >{#each buttons as b}<option value={b}>{label(b, profile.layout)} — {b}</option
                      >{/each}</select
                  >{:else if outputKind === 'trigger'}<select
                    id="output-code"
                    value={outputCode}
                    on:change={(e) => changeOutput('trigger', e.currentTarget.value)}
                    ><option value="ABS_Z">{label('ABS_Z', profile.layout)}</option><option
                      value="ABS_RZ">{label('ABS_RZ', profile.layout)}</option
                    ></select
                  >{:else if outputKind === 'dpad'}<select
                    id="output-code"
                    value={outputCode}
                    on:change={(e) => changeOutput('dpad', e.currentTarget.value)}
                    ><option value="ABS_HAT0X:-1">← Esquerda</option><option value="ABS_HAT0X:1"
                      >→ Direita</option
                    ><option value="ABS_HAT0Y:-1">↑ Cima</option><option value="ABS_HAT0Y:1"
                      >↓ Baixo</option
                    ></select
                  >{:else if outputKind === 'mouse'}<select
                    id="output-code"
                    value={outputCode}
                    on:change={(e) => changeOutput('mouse', e.currentTarget.value)}
                    >{#each ['BTN_LEFT', 'BTN_RIGHT', 'BTN_MIDDLE', 'BTN_SIDE', 'BTN_EXTRA'] as b}<option
                        value={b}>{b}</option
                      >{/each}</select
                  >{:else}<input
                    id="output-code"
                    value={outputCode}
                    placeholder="KEY_SPACE"
                    list="key-codes"
                    on:change={(e) => changeOutput('key', e.currentTarget.value.toUpperCase())}
                  /><datalist id="key-codes"
                    >{#each ['KEY_SPACE', 'KEY_ENTER', 'KEY_ESC', 'KEY_W', 'KEY_A', 'KEY_S', 'KEY_D', 'KEY_LEFTCTRL', 'KEY_LEFTSHIFT', 'KEY_E', 'KEY_F'] as k}<option
                        value={k}
                      ></option>{/each}</datalist
                  >{/if}{/if}
              <div class="binding-preview">
                <span class="small-key">{selectedLabel}</span><Icon name="remap" size={17} /><span
                  class="small-key destination"
                  >{binding.output
                    ? targetLabel(binding.output, profile.layout)
                    : label(selected, profile.layout)}</span
                ><span class="muted">evento original<br />substituído</span>
              </div>
              <div class="section-divider"></div>
              <label class="field-label" for="button-mode">COMPORTAMENTO</label>
              <div class="segmented" id="button-mode">
                <button
                  class:chosen={!binding.mode || binding.mode === 'hold'}
                  on:click={() => changeBinding({ mode: 'hold' })}>Segurar</button
                ><button
                  class:chosen={binding.mode === 'toggle'}
                  on:click={() => changeBinding({ mode: 'toggle' })}
                  >Alternar <span class="tiny-tag">TOGGLE</span></button
                >
                <button
                  class:chosen={binding.mode === 'turbo'}
                  on:click={() => changeBinding({ mode: 'turbo' })}>Turbo</button
                >
              </div>
              {#if binding.mode === 'turbo'}<label class="slider-field"
                  >Frequência <span>{binding.hz ?? 10} Hz</span><input
                    aria-label="Frequência do turbo"
                    type="range"
                    min="1"
                    max="100"
                    step="1"
                    value={binding.hz ?? 10}
                    on:input={(e) => changeBinding({ hz: +e.currentTarget.value })}
                  /></label
                >{/if}
              <p class="help">
                {binding.mode === 'turbo'
                  ? 'Repete a saída enquanto o botão físico estiver pressionado.'
                  : binding.mode === 'toggle'
                    ? 'Um toque mantém pressionado. O próximo toque solta o botão virtual.'
                    : 'A saída fica pressionada enquanto você segura o botão físico.'}
              </p>
              <button
                class="text-button"
                on:click={toggleListening}
                aria-pressed={listening}
                disabled={!connected || !controller || !!controller.error}
                ><Icon name="record" size={15} />{listening
                  ? 'Cancelar escuta · até 15 s'
                  : 'Identificar outro botão'}</button
              >
              <div class="panel-bottom">
                <span class="mono">{selected}</span><button
                  class="text-button"
                  on:click={resetBinding}>Restaurar</button
                >
              </div>
            {/if}
          </section>
        </div>
        <div class="tip-row">
          <Icon name="bolt" size={17} /><span
            ><strong>Seu próximo movimento, simplificado.</strong> Troque botões, mantenha ações pressionadas
            ou envie atalhos — tudo pelo controle virtual.</span
          ><button class="text-button" on:click={() => (page = 'diagnostics')}
            >Explorar eventos <Icon name="chevron" size={14} /></button
          >
        </div>
      {:else if page === 'profiles'}
        <div class="section-toolbar">
          <div class="inline-create">
            <input
              aria-label="Nome do novo perfil"
              placeholder="Nome do novo perfil"
              bind:value={profileName}
              on:keydown={(e) => {
                if (e.key === 'Enter') createProfile();
              }}
            /><button
              class="button primary"
              on:click={createProfile}
              disabled={!connected || !profileName.trim()}
              ><Icon name="plus" size={17} />Criar perfil</button
            >
          </div>
          <div class="row">
            <button class="button subtle" disabled={!native()} on:click={importProfile}
              ><Icon name="download" size={17} />{t.import}</button
            ><button class="button subtle" disabled={!native()} on:click={exportProfile}
              ><Icon name="upload" size={17} />{t.export}</button
            >
          </div>
        </div>
        <div class="profile-grid">
          {#each status.profiles.length ? status.profiles : [profile] as p, i}<article
              class="profile-card"
              class:is-active={controller?.profile === p.id}
            >
              <div class="profile-art" style={'--art-hue:' + ((i * 47) % 360)}>
                <Icon name="pad" size={74} /><span class="art-orbit"></span><span
                  class="art-orbit second"
                ></span>
              </div>
              <div class="profile-card-content">
                <div class="row spread">
                  <h2>{p.name}</h2>
                  {#if controller?.profile === p.id}<span class="tag green">ATIVO</span>{/if}
                </div>
                <p>
                  {Object.keys(p.buttons).length} botões · {Object.keys(p.axes).length} eixos personalizados
                </p>
                <div class="row spread">
                  <span class="mono">{p.id}</span><button
                    class="text-button"
                    disabled={!connected}
                    on:click={() => activate(p.id)}
                    >Ativar perfil <Icon name="chevron" size={15} /></button
                  >
                </div>
              </div>
            </article>{/each}
        </div>
        <section class="surface automatic-settings">
          <div class="row spread">
            <div>
              <span class="overline">TROCA AUTOMÁTICA · {profile.name}</span>
              <h2>Entre no jogo. O perfil acompanha.</h2>
              <p class="muted">
                Reconhece nomes de processos Linux e executáveis .exe do Wine/Proton.
              </p>
            </div>
            <Icon name="bolt" />
          </div>
          <div class="automatic-fields">
            <label
              >Processos (separados por vírgula)<input
                aria-label="Processos do perfil"
                placeholder="eldenring.exe, meu-jogo-linux"
                value={profile.processes?.join(', ') ?? ''}
                on:change={(e) =>
                  edit({
                    ...clone(profile),
                    processes: e.currentTarget.value
                      .split(',')
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })}
              /></label
            ><label class="priority-field"
              >Prioridade<input
                aria-label="Prioridade do perfil automático"
                type="number"
                value={profile.priority ?? 0}
                on:change={(e) => edit({ ...clone(profile), priority: +e.currentTarget.value })}
              /></label
            >
          </div>
          <div class="automatic-fields">
            <label
              >Restringir a controles (IDs ou padrões, opcional)<input
                aria-label="Controles do perfil automático"
                placeholder="054c:09cc:* · vazio aplica a todos"
                value={profile.devices?.join(', ') ?? ''}
                on:change={(e) =>
                  edit({
                    ...clone(profile),
                    devices: e.currentTarget.value
                      .split(',')
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })}
              /></label
            >
          </div>
          <p class="help">
            Aceita * e ?, sem diferenciar maiúsculas. Inspeção de processos a cada 1 s, separada do
            processamento de input. Maior prioridade prevalece. Uma escolha manual vale até mudar o
            jogo reconhecido.
          </p>
          <details>
            <summary>Processos observados nesta sessão</summary>
            <div class="process-tags">
              {#each (status.processes ?? []).slice(0, 100) as process}<span class="tag"
                  >{process}</span
                >{/each}
            </div>
          </details>
        </section>
      {:else if page === 'axes'}
        <div class="axis-grid">
          {#each axes as code}{@const b = {
              ...defaultAxis(),
              ...profile.axes[code],
              ...currentMap.axes[code],
            }}
            <section class="surface axis-card">
              <div class="row spread">
                <div>
                  <span class="overline"
                    >{code.startsWith('ABS_HAT')
                      ? 'D-PAD'
                      : code === 'ABS_Z' || code === 'ABS_RZ'
                        ? 'GATILHO'
                        : 'ANALÓGICO'}</span
                  >
                  <h2>{code}</h2>
                </div>
                <Icon name="axes" />
              </div>
              <svg
                viewBox="0 0 260 125"
                class="curve-graph"
                role="img"
                aria-label={'Curva de resposta de ' + code}
                ><path
                  d="M20 10v100h220M20 65h220M130 10v100"
                  stroke="currentColor"
                  opacity=".1"
                  fill="none"
                /><path
                  d="M20 110 240 20"
                  stroke="currentColor"
                  opacity=".15"
                  stroke-dasharray="4 5"
                /><path
                  d={curvePath(b)}
                  stroke="var(--accent)"
                  fill="none"
                  stroke-width="2.5"
                /></svg
              ><label class="slider-field"
                >Zona morta <span>{Math.round(b.deadzone * 100)}%</span><input
                  type="range"
                  min="0"
                  max="0.9"
                  step="0.01"
                  value={b.deadzone}
                  on:input={(e) => axisChange(code, { deadzone: +e.currentTarget.value })}
                /></label
              ><label class="slider-field"
                >Curva de resposta <span>{b.curve.toFixed(1)}×</span><input
                  type="range"
                  min="0.1"
                  max="4"
                  step="0.1"
                  value={b.curve}
                  on:input={(e) => axisChange(code, { curve: +e.currentTarget.value })}
                /></label
              ><label class="checkbox-row"
                ><input
                  type="checkbox"
                  checked={b.invert}
                  on:change={(e) => axisChange(code, { invert: e.currentTarget.checked })}
                />Inverter eixo</label
              ><label class="field-label" for={'mode-' + code}>DESTINO</label><select
                id={'mode-' + code}
                value={b.mode}
                on:change={(e) => {
                  const mode = e.currentTarget.value as AxisBinding['mode'];
                  axisChange(code, {
                    mode,
                    target: mode === 'mouse' ? 'REL_X' : mode === 'gamepad' ? code : null,
                  });
                }}
                ><option value="gamepad">Controle virtual</option><option value="keys"
                  >Teclas ou botão digital</option
                ><option value="mouse">Movimento do mouse</option></select
              >{#if b.mode === 'gamepad'}<select
                  aria-label={'Eixo de saída de ' + code}
                  value={b.target ?? code}
                  on:change={(e) => axisChange(code, { target: e.currentTarget.value })}
                  >{#each axes as dest}<option value={dest}>{dest}</option>{/each}</select
                >{:else if b.mode === 'mouse'}<select
                  aria-label={'Direção do mouse de ' + code}
                  value={b.target ?? 'REL_X'}
                  on:change={(e) => axisChange(code, { target: e.currentTarget.value })}
                  ><option value="REL_X">Horizontal</option><option value="REL_Y">Vertical</option
                  ></select
                ><label class="slider-field"
                  >Velocidade <span>{b.sensitivity} px/s</span><input
                    type="range"
                    min="100"
                    max="3000"
                    step="50"
                    value={b.sensitivity}
                    on:input={(e) => axisChange(code, { sensitivity: +e.currentTarget.value })}
                  /></label
                >{:else}<label class="slider-field"
                  >Limiar <span>{Math.round(b.threshold * 100)}%</span><input
                    type="range"
                    min="0.05"
                    max="1"
                    step="0.05"
                    value={b.threshold}
                    on:input={(e) => axisChange(code, { threshold: +e.currentTarget.value })}
                  /></label
                >
                <div class="row">
                  <input
                    aria-label={'Saída negativa de ' + code}
                    placeholder="KEY_A"
                    value={b.negative && 'code' in b.negative ? b.negative.code : ''}
                    on:change={(e) =>
                      axisChange(code, {
                        negative: targetFromCode(e.currentTarget.value),
                      })}
                  /><input
                    aria-label={'Saída positiva de ' + code}
                    placeholder="KEY_D / BTN_SOUTH"
                    value={b.positive && 'code' in b.positive ? b.positive.code : ''}
                    on:change={(e) =>
                      axisChange(code, {
                        positive: targetFromCode(e.currentTarget.value),
                      })}
                  />
                </div>{/if}
            </section>{/each}
        </div>
      {:else if page === 'diagnostics'}
        <div class="diagnostic-top">
          <section class="surface device-details">
            <div class="row spread">
              <div>
                <span class="overline">DISPOSITIVO FÍSICO</span>
                <h2>{controller?.device.name ?? 'Aguardando um controle'}</h2>
              </div>
              <span class="tag"
                >{controller
                  ? controller.device.vendor + ':' + controller.device.product
                  : 'EVDEV'}</span
              >
            </div>
            {#if controller}<div class="detail-grid">
                <div>
                  <small>DRIVER</small><span class="mono"
                    >{controller.device.driver || 'Não identificado'}</span
                  >
                </div>
                <div>
                  <small>HIDRAW</small><span
                    >{controller.device.hidraw_hidden
                      ? 'Acesso oculto por regra'
                      : 'Ocultação não confirmada'}</span
                  >
                </div>
              </div>
              <div class="node-list">
                {#each controller.device.nodes as n}<div>
                    <span class="node-role">{n.role}</span><code>{n.path}</code><span class="muted"
                      >{n.name}</span
                    >
                  </div>{/each}{#each controller.device.hidraw as node}<div>
                    <span class="node-role">hidraw</span><code>{node}</code>
                  </div>{/each}
              </div>
              <details>
                <summary>Identidade e topologia</summary><code class="break-all"
                  >{controller.device.id}<br />{controller.device.physical_path}</code
                >
              </details>
              <details class="capability-list">
                <summary>Capacidades evdev expostas pelo driver</summary
                >{#each controller.device.nodes as n}<h3>{n.role} · {n.path}</h3>
                  <div class="row">
                    {#each n.keys ?? [] as code}<button
                        class="capability-code"
                        on:click={() => {
                          selected = n.role === 'gamepad' ? code : `${n.role}:${code}`;
                          page = 'remap';
                        }}>{code}</button
                      >{/each}
                  </div>
                  <div class="process-tags">
                    {#each Object.entries(n.axes ?? {}) as [code, range]}<span class="tag"
                        >{code}: {range.minimum}…{range.maximum}</span
                      >{/each}
                  </div>{/each}
              </details>{:else}<p class="muted">
                Inicie padsmithd e conecte o controle. Todos os nós associados aparecerão aqui.
              </p>{/if}
          </section>
          <section class="surface rear-diagnostic">
            <span class="mini-icon"><Icon name="record" size={20} /></span>
            <h2>E os botões traseiros?</h2>
            <p>Pressione L4, R4 ou M e observe os eventos de <strong>todos os nós</strong>.</p>
            <p class="help">
              Se o botão estiver sem comando no software oficial, o firmware pode não enviar um
              evento. Receber dados dos sensores não significa que um traseiro foi reconhecido.
              Teste também um botão comum para confirmar a escuta. Configurar associações no
              firmware exige suporte específico ao protocolo do fabricante.
            </p>
            <span class="tag">{seen.size} códigos de botão observados nesta sessão</span>
          </section>
        </div>
        <section class="surface event-monitor">
          <div class="section-toolbar">
            <div class="row">
              <span class="status-dot" class:online={connected && !freeze}></span>
              <h2>Eventos ao vivo</h2>
              <span class="tag">TODOS OS NÓS</span>
            </div>
            <div class="row">
              <input
                class="search-input"
                placeholder="Filtrar código, nó ou tipo…"
                aria-label="Filtrar eventos"
                bind:value={filter}
              /><button class="button subtle" on:click={() => (freeze = !freeze)}
                ><Icon name={freeze ? 'play' : 'pause'} size={15} />{freeze
                  ? 'Continuar'
                  : 'Congelar'}</button
              ><button
                class="icon-button"
                aria-label="Limpar eventos"
                on:click={() => {
                  raw = [];
                  streamBuffer = [];
                }}><Icon name="trash" size={17} /></button
              >
            </div>
          </div>
          <div class="event-table-wrap">
            <table>
              <thead
                ><tr
                  ><th>Tempo (µs)</th><th>Nó / função</th><th>Tipo</th><th>Código</th><th>Valor</th
                  ></tr
                ></thead
              ><tbody
                >{#each filteredRaw.slice(0, 150) as event}<tr
                    class:key-event={event.event_type === 1}
                    ><td>{event.time_us}</td><td>{event.node}<small>{event.role}</small></td><td
                      >{event.event_type}</td
                    ><td>{event.label} <small>#{event.code}</small></td><td
                      ><span class:positive={event.value > 0} class="event-value"
                        >{event.value}</span
                      ></td
                    ></tr
                  >{:else}<tr
                    ><td colspan="5"
                      ><div class="empty-state">
                        <Icon name="diagnostics" size={35} /><strong
                          >{freeze ? 'Captura congelada' : 'Aguardando eventos'}</strong
                        ><span>Pressione um botão ou mova um analógico.</span>
                      </div></td
                    ></tr
                  >{/each}</tbody
              >
            </table>
          </div>
        </section>
        <section class="surface doctor-section">
          <div class="row spread">
            <div>
              <h2>Saúde do sistema</h2>
              <p class="muted">Permissões, udev, captura exclusiva e KWin.</p>
            </div>
            <button class="button primary" on:click={runDoctor} disabled={checking || !native()}
              ><Icon name="diagnostics" size={17} />{checking
                ? 'Verificando…'
                : 'Executar doctor'}</button
            >
          </div>
          {#each checks as check}<div class="doctor-check">
              <span class={'check-level ' + check.level}>{check.level}</span>
              <div>
                <strong>{check.subject}</strong>
                <p>{check.detail}</p>
              </div>
            </div>{/each}
        </section>
      {:else if page === 'settings'}
        <section class="surface settings-card">
          <div>
            <span class="overline">APARÊNCIA</span>
            <h2>Do seu jeito, até nos detalhes.</h2>
            <p class="muted">O tema automático acompanha a preferência do sistema.</p>
          </div>
          <div class="theme-options">
            {#each [['system', 'Sistema'], ['dark', 'Escuro'], ['light', 'Claro']] as [value, name]}<button
                class:chosen={theme === value}
                on:click={() => updateTheme(value)}
                ><div class={'theme-preview ' + value}><span></span><i></i><b></b></div>
                <span>{name}</span>{#if theme === value}<Icon name="check" size={15} />{/if}</button
              >{/each}
          </div>
          <div class="section-divider"></div>
          <label class="field-label" for="layout">RÓTULOS DO CONTROLE</label><select
            id="layout"
            value={profile.layout}
            on:change={(e) => edit({ ...clone(profile), layout: e.currentTarget.value })}
            ><option value="xbox">Xbox — A / B / X / Y</option><option value="playstation"
              >PlayStation — × / ○ / □ / △</option
            ><option value="nintendo">Nintendo — B / A / Y / X</option></select
          >
          <p class="help">Os códigos posicionais permanecem os mesmos em todos os perfis.</p>
          <div class="section-divider"></div>
          <label class="field-label" for="locale">IDIOMA</label><select id="locale"
            ><option>Português (Brasil)</option></select
          >
        </section>
        <div class="banner">
          <Icon name="info" /><span
            >O remapeamento usa dispositivos virtuais do Linux. Mantenha o plugin Game Controller do
            KWin desativado. O diagnóstico indica problemas de acesso e captura.</span
          >
        </div>
        <Layers
          {profile}
          profiles={status.profiles}
          {edit}
          learn={(callback) => (learnCallback = callback)}
        />
      {:else if page === 'macros'}
        <Macros
          {profile}
          {connected}
          device={selectedDevice}
          recording={!!controller?.recording}
          {edit}
          fail={(message) => (error = message)}
          notice={notify}
          learn={(callback) => (learnCallback = callback)}
        />
      {/if}
      <footer class="main-footer">
        <span>padsmith<span class="brand-dot">.</span></span><span
          >Projetado para o Linux. Moldado para você.</span
        >
      </footer>
    </main>
  </div>
</div>
{#if toast}<div class="toast" role="status"><Icon name="info" size={18} />{toast}</div>{/if}
