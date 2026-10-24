---
name: padsmith-project
description: Use when discussing, explaining, debugging, extending, testing, or packaging the Padsmith repository; covers its Rust evdev/uinput daemon, pure mapping engine, Tauri/Svelte UI, profiles, macros, diagnostics, and NixOS integration.
---

# Padsmith — contexto do projeto

Use esta skill como guia interno para entender o Padsmith e para responder perguntas
sobre seu funcionamento ou modificar o repositório. Leia os arquivos atuais antes de
depender de um detalhe: o código e as configurações presentes neste checkout são a
fonte de verdade; esta explicação é um mapa, não substitui inspeção do comportamento.
Responda em português brasileiro, salvo pedido explícito por outro idioma.

## Em uma frase

Padsmith é um remapeador de controles para Linux: um daemon de usuário agrupa e toma
posse exclusiva dos nós evdev de um controle físico, transforma eventos segundo um
perfil, e publica saídas por controles virtuais uinput — um gamepad virtual com IDs
Xbox 360 `045e:028e`, além de teclado e mouse virtuais. CLI e interface gráfica são
clientes separados; fechar a interface não encerra o daemon.

O objetivo é fazer Steam/SDL/Proton consumirem o controle virtual remapeado e, quando
configurado, impedir o acesso direto da Steam ao hidraw físico. Os IDs de uinput, por
si sós, não provam que todo jogo o reconhecerá como XInput: isso depende do kernel,
Steam, SDL, Proton e do ambiente real.

## Arquitetura do workspace

```text
evdev físico ── udev/hotplug ── padsmithd ── padsmith-core (transições puras)
                                      │
                                      ├── uinput: gamepad virtual por controle
                                      ├── uinput: teclado virtual compartilhado
                                      ├── uinput: mouse virtual compartilhado
                                      └── socket Unix em $XDG_RUNTIME_DIR/padsmith/
                                             ├── CLI padsmith
                                             └── interface Tauri/Svelte
```

- `crates/core`: tipos TOML e máquina de estados determinística; sem relógio, thread,
  filesystem ou syscalls. `step(state, profile, input, now_us)` retorna estado novo,
  eventos virtuais, prazo seguinte e efeitos de controle.
- `crates/linux`: descoberta/agrupamento udev, normalização evdev, `EVIOCGRAB`,
  dispositivos uinput, validação/salvamento de perfis, processos e `doctor`.
- `crates/protocol`: protocolo IPC JSON por linha, versão, limite de mensagem de
  1 MiB, tipos compartilhados e resolução de config/socket XDG.
- `crates/daemon`: hotplug, sessões por controle, leitura de eventos, relógios de
  macros/turbo, IPC, perfis, gravação, processos, notificações e watchdog systemd.
- `crates/cli`: executável `padsmith` (`devices`, `doctor`, `monitor`, `profile`,
  `pause`, `resume`, `record`).
- `crates/ui`: aplicação Tauri 2 (Rust) e bandeja; exporta comandos para o frontend.
- `ui`: Svelte + TypeScript + Vite, telas, controle vetorial e editor.
- `nix/` + `flake.nix`: pacotes, frontend pnpm, módulo NixOS e devShell.

Para entender um fluxo, leia a implementação nas duas pontas e o tipo IPC entre elas;
não inferir comportamento apenas pelo nome de um controle da interface.

## Fluxo de entrada, captura e saída

1. `devices::discover()` enumera eventos de input e os agrupa pela ancestralidade HID
   ou, quando ela não existe, pela topologia/serial disponível. Inclui irmãos de
   sensores, touchpad e outros nós, ainda que não sejam joystick. Enumera também
   hidraw e `power_supply` quando associados.
2. VID/PID/serial e conexão/topologia geram uma identidade física. Números `eventN`
   são caminhos transitórios, não identidade persistente. Conferir `padsmith devices
   --json` ao investigar mudança de modo/receptor ou perfis não associados.
3. A preparação de dispositivos acontece fora do loop principal: reabre cada nó,
   verifica nome/VID/PID/serial para reduzir risco de corrida de hotplug e aplica
   `EVIOCGRAB`. Se qualquer nó falhar, os grabs parciais são liberados e a captura
   inteira falha. O frontend deve exibir claramente esse erro.
4. Cada nó tem um reader orientado a eventos, sem polling de evdev. Teclas viram
   `Input::Button`; eixos absolutos são normalizados pelo mínimo/máximo para `[-1,1]`
   ou `[0,1]` nos gatilhos do gamepad. Entradas em nós auxiliares recebem prefixo,
   por exemplo `touchpad:...` ou `motion:...`. Eventos crus também são publicados
   para diagnóstico, através de uma fila de broadcast limitada.
5. O engine puro resolve fontes, bindings, camadas, gestos, macros, turbo, deadzone,
   curvas e inversão. Ownership é por fonte: soltar o controle não solta uma saída
   que outra fonte (macro, eixo, toggle etc.) ainda mantém pressionada.
6. O daemon emite eventos uinput e timers de deadline. Macros/turbo/mouse usam timers
   somente quando necessários; não existe busy loop para input. Uma perda `SYN_DROPPED`
   neutraliza as saídas e tenta ressincronizar eixos absolutos.

### Dispositivos virtuais

- Um gamepad virtual é criado por controle físico. Ele apresenta `BUS_USB`, VID `045e`,
  PID `028e`, botões posicionais e eixos padrão listados em `crates/core/src/lib.rs`.
- Teclado e mouse virtuais são compartilhados entre controles; `Desktop` acompanha
  os proprietários de cada tecla/botão para coordenar liberações.
- A identidade declarada como Xbox 360 é uma identidade de dispositivo evdev/uinput,
  não um protocolo USB XInput implementado por firmware.
- Não há envio de comandos ao firmware e não há repasse de force feedback/vibração.
- Não anunciar emulação de giroscópio/touchpad como capacidade do Xbox virtual. Eventos
  de nós auxiliares são diagnosticáveis e códigos de tecla podem ser associados;
  uma API de sensores virtual exigiria implementação própria.

## Mapeamento e rótulos

Os códigos internos são posicionais e não mudam com o rótulo do usuário:

| Código Linux | Xbox | PlayStation | Nintendo |
|---|---|---|---|
| `BTN_SOUTH` | A | × | B |
| `BTN_EAST` | B | ○ | A |
| `BTN_WEST` | X | □ | Y |
| `BTN_NORTH` | Y | △ | X |

Na física Linux, **X é normalmente `BTN_WEST` e Y `BTN_NORTH`**. A troca X ↔ Y deve
descartar a saída original e emitir somente o destino posicional correspondente.
Nunca deduzir códigos pela letra impressa no hardware sem consultar os eventos reais.

Exceção na fronteira de saída: o gamepad virtual com identidade Xbox 360 segue a
ABI histórica do `xpad`, em que X = `BTN_X`/`BTN_NORTH` (307) e Y =
`BTN_Y`/`BTN_WEST` (308). `crates/linux/src/output.rs` converte os destinos
posicionais para esses códigos somente ao anunciar capacidades/emitir no uinput.
Não propagar essa conversão para `key_name`, normalização de entrada, core, UI ou
perfis. Testar tanto o perfil padrão quanto a troca explícita X/Y contra os códigos
numéricos da ABI; comparar apenas strings do engine não detecta inversão nos jogos.

Tipos `Target` no engine: botão virtual (`gamepad`), gatilho analógico virtual
(`trigger` → `ABS_Z`/`ABS_RZ`), direção do d-pad (`dpad` → `ABS_HAT0X/Y`), tecla,
botão do mouse ou `disabled`. Botões sem regra usam destino padrão somente para o
conjunto conhecido. Códigos extras precisam de uma associação explícita.

Gatilhos digitais Linux `BTN_TL2`/`BTN_TR2` viram gatilhos virtuais analógicos. D-pad
digital vira eixo hat. A saída tem propriedade compartilhada: duas fontes não devem
zerar o gatilho/d-pad enquanto uma delas ainda o mantém ativo.

Eixos são configuráveis como controle, teclas/botões digitais ou mouse: target,
inversão, deadzone, curva exponencial, limiar e sensibilidade. Mouse usa velocidade
em pixels/segundo e ticks de aproximadamente 8 ms enquanto ativo. Se ajustar os
limites ou a normalização, rever os testes unitários de eixos e integração uinput.

## Toggle, turbo, macros e gravação

- `hold`: saída enquanto a entrada física estiver pressionada.
- `toggle`: só a borda de pressionar alterna ownership; a soltura física não desfaz
  o toggle. Novo toque solta. Reset, pausa, troca de perfil e desconexão limpam estado.
- `turbo`: alternância com duty cycle de 50%, frequência configurável entre 1 e
  100 Hz (`hz`), avançada por deadlines; não gerar rajadas para compensar atraso.
- Passos de macro: `press`, `release`, `hold`, `wait`, `repeat`, `axis`, `move`.
  Repetições são expandidas com limites de profundidade, ações, duração e quantidade.
- Gatilhos: `tap`, `long_press`, `double_tap`, `chord`, `while_held`, `toggle_loop`.
  `consume=true` reserva os botões e impede que seus eventos originais também atuem.
- Macros têm um ownership próprio. Cancelamento/fim libera apenas suas saídas, sem
  desfazer bindings independentes. Testar sobreposição de fontes ao mudar o engine.
- Durante gravação, o daemon neutraliza o controle virtual e não emite os eventos
  gravados. Grava passos mapeáveis com tempos; entradas auxiliares sem saída conhecida
  são reportadas como omitidas, não adivinhadas. No fim, acrescenta solturas/neutralização.
- CLI: `padsmith record start <ID>` e `padsmith record stop <ID>`.

## Perfis, camadas, teclas de emergência e jogos

Perfil é TOML versão 1 e contém mapa de botões/eixos, macros, triggers, aliases para
apresentação dos traseiros, seleção por processos/dispositivos, prioridade, camadas,
atalhos e emergência. Um arquivo por perfil em:

```text
$XDG_CONFIG_HOME/padsmith/profiles/*.toml
# fallback: ~/.config/padsmith/profiles/*.toml
```

`assignments.toml` registra escolha manual por ID e conexão física, além do perfil
padrão. Arquivos TOML são observados com watcher; só aplicar ao vivo depois de validar.
A gravação usa escrita atômica, `fsync` e rename. Importação/exportação são clientes
IPC e a versão do protocolo daemon/CLI/GUI precisa permanecer alinhada.

Camadas têm ativadores, prioridade, mapa de botões e eixos. São reavaliadas em
transições de botão/eixo: neutralizam os destinos anteriores e reavaliam as entradas
que ainda estão pressionadas. Associações da camada prevalecem; faltas herdam o mapa
base. Empate é resolvido deterministicamente pelo ID.

Emergência padrão: segurar `BTN_MODE` + `BTN_SELECT` por 3000 ms para alternar pausa.
Continua monitorada mesmo pausado; neutraliza saídas e encerra gravação. Atalhos de
perfil também são combinações pressionadas por duração; `$next` percorre IDs de perfis.

Seleção automática varre `/proc` do mesmo UID aproximadamente a cada 1 s, fora do
caminho de input, e casa nomes com `*`/`?`, sem diferenciar maiúsculas. Lê nome do
executável e argumentos `.exe` do Wine/Proton; isso deve ser validado em jogo real,
pois o teste usa um executável sintético. Maior `priority` vence. Um perfil pode
restringir `devices` a IDs/padrões. Ao cessar o match, volta ao perfil manual anterior.

## Diagnóstico e segurança de hardware

`padsmith devices` lista identidade, driver, IDs, nós e hidraw. A tela Diagnóstico e
`padsmith monitor` mostram eventos crus (tipo, código, valor, nó); `padsmith doctor`
verifica `/dev/uinput`, leitura dos evdev irmãos, estado das regras, acesso hidraw,
KWin e tentativa breve de grab. `EBUSY` não revela o processo proprietário ao kernel.

Não alegar que L4/R4/M são eventos separados antes de observá-los. Um alias de traseiro
na interface guarda a associação visual; o motor mapeia o código evdev real. Se o
firmware multiplexa dois botões no mesmo código, evdev não permite distingui-los.
Explicar quando a captura não observou evento ou quando o código coincide com outro.

`EVIOCGRAB` sequestra evdev, **não** hidraw. Opcionalmente, o módulo instala regras:

- `70-padsmith-uinput.rules`: acesso `uaccess` ao `/dev/uinput`.
- `71-padsmith-input.rules`: `uaccess` aos nós input dos seletores VID/PID.
- `72-padsmith-hidraw.rules`: ocultação opcional, antes de `73-seat-late.rules`.

Hidraw restrito permanece restrito mesmo com o daemon parado; descritores que a Steam
já abriu podem exigir reiniciar a Steam e reconectar. Não retirar driver HID do kernel.
Não colocar o usuário no grupo global `input` como atalho. Não enviar feature reports,
comandos de firmware ou instruções para alterar modo do controle por USB.

KWin **Game Controller** pode traduzir controles em mouse/teclado, inclusive uinput.
Manter `gamecontrollerEnabled=false` e verificar sessão/D-Bus. O antigo
`gamepad-desktop-guard` pode abrir nós em leitura, mas não faz grab e não substitui o
Padsmith; ao alterar serviço de usuário, inspecionar o módulo NixOS do usuário antes.

## IPC e ciclos de vida

- Socket `$XDG_RUNTIME_DIR/padsmith/control.sock`, protegido por diretório `0700`,
  socket `0600`, credenciais UID e versão IPC.
- Clientes: `Status`, `SaveProfile`, `Apply`, `Pause`, `Watch`, `RecordStart/Stop`,
  `RunMacro`. `Watch` recebe status e eventos crus/avisos; filas limitadas sinalizam
  mensagens perdidas sem bloquear o daemon.
- Tauri encaminha comandos Rust para o socket e retransmite mensagens via eventos para
  Svelte. Se o daemon cair, GUI tenta reconectar. Fechar janela esconde-a; “Sair da
  interface” encerra GUI, não daemon.
- Daemon é serviço systemd **de usuário**, sem root. `Type=notify`, watchdog de 5 s,
  SIGKILL no timeout e restart ajudam a recuperar travamentos. Encerrar o processo fecha
  os FDs e libera os grabs. Não introduzir operações de disco/processos no caminho de
  input; preparação udev e perfis devem ficar fora dele.

## Estrutura de interface

`ui/src/App.svelte` monta as páginas; `components/Controller.svelte` é a ilustração
interativa, `Macros.svelte` a timeline e `Layers.svelte` edita mapas/atalhos.
`ui/src/lib/types.ts` replica os tipos serializados do Rust e `bridge.ts` implementa
IPC/eventos Tauri. `i18n.ts` é a base dos dicionários; strings atuais são pt-BR.
`crates/ui/src/tray.rs` mantém menu da bandeja e seleção de perfil.

Ao alterar o contrato Rust (`Profile`, `Command`, `Message`, status), atualizar tipos
TypeScript e clientes. Ao mudar identificadores posicionais, atualizar desenho,
rótulos Xbox/PS/Nintendo, TOML de exemplo, validação, testes e docs.

## NixOS e desenvolvimento

`flake.nix` exporta `packages.daemon`, `frontend`, `ui`, `default`, módulo NixOS e
`devShells.default`. O pacote default junta daemon/CLI com Tauri GUI. O módulo
`services.padsmith` instala pacote, `uinput`, regras udev e serviços systemd do usuário.
Leia `docs/nixos.md` antes de aconselhar instalação; o switch é responsabilidade do
usuário. Nesta instalação original existe uma configuração NixOS externa em
`~/.nixos-config`, mas nunca presumir que todo clone possui esse caminho. Inspecione
o flake/módulos reais antes de modificar configuração pessoal; não executar `sudo`
ou `nixos-rebuild` sem pedido explícito.

Comandos úteis, na raiz e normalmente dentro de `nix develop`:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
nix flake check path:.
nix build 'path:.#default'
padsmith devices --json
padsmith doctor
padsmith monitor --json
```

Os testes comuns não precisam de hardware. Testes opt-in de daemon criam controles
uinput e exigem ACLs de `/dev/uinput`/evdev; nunca os execute com uma segunda instância
do daemon competindo pelos fixtures. Veja `docs/validacao.md` para passos manuais.

## Testes e definição de evidência

- Core: testes de transição para remapeamento, ownership, toggle/turbo, curvas, macros,
  triggers, camadas e emergência em `crates/core/tests/`.
- Linux: normalização e perfis de exemplo em `crates/linux/tests/`.
- Integração opt-in: `crates/daemon/tests/uinput.rs`; fonte synthetic uinput → daemon
  real → gamepad virtual lido; também hotplug, múltiplos controles, processo `.exe`
  sintético e soltura de grabs após sinais.
- UI: `pnpm --dir ui build` cobre typecheck/compiler; verificação browser IPC-simulated
  não comprova hardware, XInput em jogos nem bandeja real.
- Nix: `nix flake check path:.` avalia módulo, verifica regras e compila os pacotes.
- Métricas de latência citadas em docs medem uinput de entrada → daemon → uinput saída;
  não incluem rádio, taxa do controle, compositor ou jogo. Não transformar resultado
  medido em garantia universal de tempo real.

Ao explicar status de validação, separar teste puro, integração sintética, build e
teste físico. Não dizer “controle funciona” só porque compilou ou o fixture funcionou.

## Guia para futuras alterações

1. Identifique o componente que possui o comportamento (core, Linux, daemon, IPC, UI,
   Nix). Confirme formatos com código atual antes de mudar contratos.
2. Mantenha o motor puro e determinístico; introduza relógio somente como `now_us`
   e deadlines explícitos. Nunca adicionar polling ativo de eventos evdev.
3. Preserve exclusividade/recuperação: em erro parcial, rollback de todos os grabs;
   em reset, pausa, troca de perfil, gravação, hotplug e saída, neutralizar saídas
   sem soltar ownership ainda mantido por outra origem.
4. Eventos crus observados são evidência; nomes de botão comercial, manual online ou
   relato da comunidade não substituem captura do hardware.
5. Validar configuração antes de persistir/aplicar; manter TOML legível/editável,
   versionado e escrito atomicamente.
6. Em mudanças Rust, rodar testes e Clippy apropriados; em UI, typecheck/build; em Nix,
   avaliar módulo/flake. Usar integração sintética quando tocar em capture/output,
   declarando claramente se permissões impediram sua execução.
7. Atualizar README/docs, exemplos e esta skill quando arquitetura/limitações mudarem.

## Leituras de referência

- `README.md`: overview, funcionalidades, execução, limitações.
- `docs/validacao.md`: testes, diagnóstico e roteiro físico.
- `docs/nixos.md`: instalação/regras/service do módulo.
- `crates/core/src/lib.rs`, `controls.rs`, `macros.rs`: contrato do engine.
- `crates/linux/src/devices.rs`, `output.rs`, `doctor.rs`: integração Linux.
- `crates/daemon/src/main.rs`: orquestração/ciclo de vida.
- `crates/protocol/src/lib.rs`: contrato IPC/config.
- `crates/ui/src/`, `ui/src/`: GUI/tray/frontend.
- `nix/module.nix`, `nix/*.nix`, `flake.nix`: empacotamento.
