# Validação e diagnóstico

## Verificações automatizadas

Dentro de `nix develop`, na raiz:

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
cargo test -p padsmith-linux --test output -- --ignored --nocapture
cargo test -p padsmithd --test uinput -- --ignored --nocapture --test-threads=1
nix flake check path:.
nix build 'path:.#default'
```

Os testes comuns não precisam de hardware. Os testes uinput são opt-in e precisam
de acesso ao dispositivo e da ACL `uaccess` para ler os gamepads sintéticos; a
sandbox do build Nix não disponibiliza esses dispositivos.
O teste Linux `output` exercita normalização → core → uinput → leitura evdev para
as quatro posições e a troca X/Y. Ele cria somente uma saída virtual excluída da
descoberta física, sem iniciar outro daemon, e pode rodar com o serviço em uso.

### O que os testes cobrem

- Motor puro: X ↔ Y sem evento original duplicado, toggle, fontes simultâneas,
  turbo por deadlines, curvas/deadzone, gatilho digital/analógico, d-pad digital,
  mouse com parada/retomada, sequências/repetições, toque longo/duplo, chords,
  loops, cancelamento, camadas, emergência e atalhos de perfil.
- Linux: nomes de todos os códigos evdev de tecla, normalização de sensores,
  exemplos TOML, reconhecimento de `.exe`/prioridades/restrições por dispositivo
  e conversão de posições para os códigos X/Y da ABI Xbox 360 (com e sem remapeamento).
- Integração: daemon real, fontes uinput isoladas, leitura dos gamepads de saída,
  IDs `045e:028e`, gravação sem injetar saídas, macros, hotplug concorrente,
  dois controles, troca de perfil por processo sintético, emergência, SIGTERM e
  SIGKILL. O processo de teste chamado `.exe` é um ELF Linux; Wine/Proton reais
  precisam da verificação manual descrita abaixo.
- Nix: construção do pacote, avaliação do módulo e `udevadm verify` nas regras.
- Frontend: Svelte/TypeScript/build, mais interações no navegador com IPC simulado.
  Esse teste de interface não é um teste do controle físico ou da bandeja KDE.

Os fixtures têm nomes reservados e o daemon dos testes usa seletores explícitos.
Se uma instância anterior capturar esses dispositivos, o teste falha com `EBUSY`.
Encerre/reinicie essa instância de forma coordenada; não pare outros programas
sem identificar a situação. A CLI `padsmith doctor` detecta a captura, mas o
kernel não identifica seu dono pelo ioctl.

## Latência

O teste imprime p50, p95, p99 e máximo do percurso **uinput de entrada → daemon →
uinput de saída**, medindo 256 pressões enquanto outro dispositivo é criado e
removido. Na execução de desenvolvimento após separar os workers de IPC, foram
observados **77 µs p50, 107 µs p95, 121 µs p99 e 147 µs de máximo**.

Essa medição não inclui rádio/receptor, taxa de relatórios do controle, frame do
jogo ou compositor. O Linux de uso geral não fornece uma garantia absoluta de
tempo real; repita o teste sob sua carga habitual. O mouse contínuo usa atualização
de velocidade a 125 Hz; isso é separado do processamento imediato do gamepad.

## Verificação no seu controle

1. Aplique o módulo conforme [nixos.md](nixos.md), reconecte o receptor e reinicie
   a Steam para fechar descritores hidraw antigos.
2. Execute `padsmith doctor`. Confira leitura de **todos** os nós, escrita em
   `/dev/uinput`, captura e acesso hidraw negado quando a regra opcional estiver
   habilitada. Um erro de sensores/touchpad impede captura parcial do controle.
3. Abra a interface e vá a Diagnóstico. Confira nome, VID/PID, driver, conexão,
   evdev e hidraw. Para o Cyclone 2 no modo PS4, confirme no próprio ambiente se
   os nós de gamepad, sensores e touchpad estão presentes.
4. Pressione cada botão individualmente. L4/R4/M devem ser investigados em todos
   os nós. Observe **tipo, código e valor**, incluindo a soltura. Compare o código
   do traseiro com os demais botões: repetir `BTN_SOUTH`, por exemplo, não comprova
   um botão traseiro independente.
5. Associe um traseiro somente ao evento observado. Se não aparecer evento
   independente, registre essa limitação do modo atual; não há como criar uma
   entrada separada que o firmware não expõe ao evdev.
6. Importe `examples/swap-xy.toml` e ative `swap-xy`. No teste de controle da Steam,
   confirme X → Y, Y → X, ausência de controle físico duplicado e toggle de A.
   Confira também um perfil sem remapeamentos: esquerda (quadrado no PS4) → X,
   cima (triângulo) → Y, baixo (×) → A e direita (círculo) → B. Na captura evdev
   da saída virtual Xbox, X é código 307 (`BTN_NORTH`/`BTN_X`) e Y é 308
   (`BTN_WEST`/`BTN_Y`), conforme `xpad`; esses aliases não são as posições do perfil.
7. Em um jogo Proton, confirme que ele recebe o controle virtual e que o
   remapeamento vale no jogo. Reconhecimento por Steam/SDL/Proton não é comprovado
   somente pela identidade VID/PID do dispositivo virtual.
8. Teste pausa/retomada, Home + Select por 3 s, fechar a janela, sair da interface,
   reconectar o receptor e trocar de modo. Compare `id` e `connection` em
   `padsmith devices --json` para conferir a recuperação do perfil.
9. Verifique a bandeja e as notificações na sessão Plasma. Ações da bandeja usam
   seu menu, compatível com a implementação Linux do Tauri.

Para salvar uma captura bruta pelo terminal:

```sh
padsmith monitor --json > captura-controle.jsonl
```

O arquivo pode ficar grande com sensores; encerre com Ctrl+C. As linhas incluem
identidade do controle e nó, para não misturar eventos de dispositivos distintos.
O monitor informa perda de mensagens sem bloquear o remapeamento.

## Watchdog

O módulo declara `Type=notify`, `WatchdogSec=5s`, `WatchdogSignal=SIGKILL` e
`Restart=on-failure`. O heartbeat vem do loop de input. A validação de um serviço
de usuário deve conferir `WatchdogTimestampMonotonic` e o resultado `watchdog`
de uma instância de teste deliberadamente suspensa, com socket/config separados
e seletor que não capture controles reais. Não suspenda a instância usada para jogar.

Essa verificação também foi executada: uma unidade transitória isolada, com
watchdog de 2 s e sem capturar controles físicos, recebeu SIGSTOP. O systemd
encerrou o processo com **`Result=watchdog`, `ExecMainStatus=9`**. A unidade de
desenvolvimento continuou ativa com heartbeat atualizado; a unidade de teste foi
limpa depois da inspeção.

## Limitações explicitamente verificáveis

- Hidraw permanece restrito com o daemon parado quando a regra opcional está ativa.
- Identificação por conexão depende dos metadados expostos pelo kernel/udev;
  dispositivos compostos e receptores devem ter seu agrupamento conferido.
- Não há comandos de configuração de firmware nem repasse de force feedback.
- Teclas/sensores/touchpad não viram capacidades inexistentes do Xbox 360 virtual.
- Gravação tem limite de passos e informa entradas omitidas sem destino mapeável.
- Os testes sintéticos comprovam o software e o caminho evdev/uinput, não o que
  L4/R4/M enviam, nem o comportamento de um jogo/driver real.
