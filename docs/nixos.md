# Instalação NixOS

O módulo instala o pacote, carrega `uinput`, instala regras udev e habilita o
serviço **de usuário** `padsmith.service`, ligado à sessão gráfica.
O pacote padrão inclui daemon, CLI, interface e entrada no menu. O serviço de
usuário `padsmith-tray.service` inicia a bandeja; `services.padsmith.tray.enable =
false` desativa esse início automático.

## Seu `~/.nixos-config/flake.nix`

Enquanto o repositório estiver apenas local, adicione aos `inputs`:

```nix
padsmith = {
  url = "path:/home/matheus/Projetos/Rust/padsmith";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

Inclua `padsmith` no argumento de `outputs`:

```nix
outputs = { nixpkgs, home-manager, nix-flatpak, padsmith, ... }: {
  # ...
};
```

E, na lista `nixosConfigurations.matheus-pc.modules`, acrescente:

```nix
padsmith.nixosModules.default
```

Quando publicar seu próprio repositório, substitua o URL local por
`github:SEU_USUARIO/padsmith`. Atualize o lock do input quando mudar a fonte local.
O arquivo `flake.lock` do Padsmith deve acompanhar o repositório publicado.

Se estiver usando a instância temporária iniciada durante o desenvolvimento,
pare-a antes de ativar o serviço definitivo para liberar o socket e os dispositivos:

```sh
systemctl --user stop padsmith-development.service
```

Essa unidade é transitória; não substitui a instalação declarativa pelo módulo.

## Seu módulo de jogos ou `modules/padsmith.nix`

```nix
{ ... }: {
  services.padsmith = {
    enable = true;
    devices = [
      { vendor = "054c"; product = "09cc"; } # Cyclone 2 em modo PS4
      { vendor = "057e"; product = "2009"; } # modo Switch, se usado
    ];
    hideHidraw = true;
    logLevel = "info";
    tray.enable = true;
  };
}
```

Se criar um arquivo novo, acrescente-o aos imports da sua `configuration.nix`.
Seletores são explícitos: outra pessoa com um controle diferente deve acrescentar
o VID/PID mostrado por `padsmith devices`. Isso concede acesso também a sensores
e touchpad, que normalmente não têm `ID_INPUT_JOYSTICK`. O daemon continua genérico
e descobre outros joysticks; os seletores não são uma lista de captura obrigatória.
`pathPattern` pode restringir uma regra a uma topologia específica, usando o
DEVPATH udev (sem o prefixo `/sys`). Sem essa restrição, VID/PID seleciona todos os
controles desse modelo. Um controle sem ancestral HID pode ser gerenciado pelo
daemon, mas exigirá uma regra evdev apropriada se o padrão do sistema não bastar.

Você aplica essas mudanças ao sistema. Depois reconecte o receptor e reinicie a
Steam para fechar descritores hidraw antigos. Em sua sessão:

```sh
systemctl --user status padsmith
systemctl --user status padsmith-tray
padsmith doctor
journalctl --user -u padsmith -f
```

## Por que as regras têm esses nomes

- `70-padsmith-uinput.rules`: `TAG+="uaccess"` para `/dev/uinput`.
- `71-padsmith-input.rules`: `uaccess` nos nós evdev irmãos selecionados.
- `72-padsmith-hidraw.rules`: opcional; remove `uaccess`, força dono/grupo root e
  modo `0600`, marca `PADSMITH_HIDRAW_HIDDEN=1` para o diagnóstico.
- `73-seat-late.rules`: executa o builtin que aplica ACLs de `uaccess`.

São pacotes em `services.udev.packages`. Não usamos `services.udev.extraRules` para
ocultação: no NixOS elas viram `99-local.rules`, tarde demais para impedir a ACL.
A regra preserva o driver do kernel; não bloqueia `hid-playstation`/`hid-nintendo`.

**A restrição hidraw persiste com o daemon parado.** Para restaurar hidraw,
desative `hideHidraw`, aplique sua configuração, reconecte o controle e reinicie
os aplicativos que o utilizam. Com o daemon parado, o evdev físico fica livre.
Reaplicar regras não revoga descritores já abertos; reconectar é necessário para
validar permissões limpas. Não adicione o usuário ao grupo global `input`.

## KWin e `gamepad-desktop-guard`

Seu `kwinrc` inspecionado contém `gamecontrollerEnabled=false`. Mantenha o plugin
Game Controller desativado nas configurações do Plasma; o grab físico não impede
que o plugin use os dispositivos **virtuais**. O `doctor` informa a configuração
gravada e tenta consultar a sessão pelo D-Bus, sinalizando quando não puder confirmar.
Nesta máquina, a propriedade `org.kde.KWin.Plugins.LoadedPlugins` também foi
consultada: o plugin Game Controller não estava carregado na sessão inspecionada.

O serviço `gamepad-desktop-guard` atual não faz grab e pode coexistir. Depois de
validar Padsmith e manter o plugin desativado, remova seu bloco ou acrescente, no
mesmo módulo que recebe `lib` nos argumentos:

```nix
systemd.user.services.gamepad-desktop-guard.enable = lib.mkForce false;
```

## Build sem instalar no sistema

```sh
nix develop
nix build 'path:.#default'
nix flake check path:.
```

Em um checkout Git, os arquivos novos precisam estar incluídos no repositório
para serem vistos pelo mecanismo de flakes. Para validar uma árvore local ainda
não adicionada ao Git, use o prefixo `path:.` como nos comandos acima.
