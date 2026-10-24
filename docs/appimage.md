# Padsmith em AppImage

O AppImage contém a interface, o daemon `padsmithd`, a CLI `padsmith` e as
bibliotecas empacotadas pelo Tauri. Abrir o arquivo reutiliza um daemon ativo;
se não houver um, inicia o daemon embutido. O daemon usa uma montagem separada
do mesmo AppImage e continua funcionando quando a interface é encerrada.

## Gerar

Na raiz, entre no ambiente do projeto e use o atalho:

```sh
nix develop
build-appimage
```

O comando é fornecido pelo `flake.nix`, inclui Podman e só fica disponível nesse
ambiente. Depois de entrar na raiz, também funciona ao navegar para subpastas do
projeto. Para executar sem abrir um shell interativo:

```sh
nix develop --command build-appimage
```

Fora do ambiente Nix, com Podman ou Docker disponível, use
`bash scripts/build-appimage.sh`.

O build usa Debian 12 em um container, incluindo o frontend e todos os binários.
A fonte é montada somente para leitura; os arquivos são compilados em uma cópia
temporária, sem substituir os executáveis ou dependências do ambiente Nix.
São necessários acesso à rede e espaço para as dependências do container.
`CONTAINER_ENGINE=docker` ou `CONTAINER_ENGINE=podman` escolhe o engine.
O cache de compilação fica em `target/appimage/.cache/`; ele não precisa ser
distribuído junto do AppImage e pode ser removido para liberar espaço.
Os diretórios `target/debug`, `target/release`, `ui/dist` e `ui/node_modules`
também são regeneráveis e não são necessários para executar o AppImage pronto.
Depois de remover o cache, a próxima compilação será completa e recriará os
arquivos de build necessários.

O resultado fica em `target/appimage/`, junto do checksum e das regras udev:

```text
Padsmith_0.1.0_amd64.AppImage
SHA256SUMS
70-padsmith-uinput.rules
71-padsmith-input.rules
README.md
nixos.md
```

A arquitetura acompanha a máquina que faz o build. O baseline é Debian 12
(glibc 2.36); AppImage não torna o programa compatível com distribuições mais
antigas que as bibliotecas básicas usadas na compilação.

## Limpar depois da compilação

No mesmo ambiente do projeto, compile e depois libere o espaço dos arquivos
temporários:

```sh
nix develop
build-appimage
clean-project
```

Também é possível limpar sem abrir um shell interativo:

```sh
nix develop --command clean-project
```

O comando remove `target/debug`, `target/release`, `target/appimage/.cache`,
`target/appimage-sidecars`, `ui/node_modules`, `ui/dist`, `.playwright-mcp` e os
metadados regeneráveis do Cargo. Os AppImages, checksums, regras udev e documentos
em `target/appimage/` são preservados. A próxima build recriará o cache e será
completa. Assim como `build-appimage`, o atalho funciona a partir de subpastas
depois de entrar em `nix develop` pela raiz.

Fora do ambiente Nix, use `bash scripts/clean-project.sh`. Para consultar a ajuda,
execute `clean-project --help`.

## Executar

```sh
chmod +x Padsmith_0.1.0_amd64.AppImage
./Padsmith_0.1.0_amd64.AppImage
./Padsmith_0.1.0_amd64.AppImage --cli status
./Padsmith_0.1.0_amd64.AppImage --cli doctor
./Padsmith_0.1.0_amd64.AppImage --daemon
```

`--daemon` é opcional: use para iniciar somente o serviço em primeiro plano,
por exemplo para diagnóstico. Não inicie outra instância se já houver um daemon.
`--cli --help` mostra os comandos da CLI; `--help` mostra os modos do AppImage.
Os perfis continuam em `$XDG_CONFIG_HOME/padsmith` (ou `~/.config/padsmith`).
Os logs do daemon iniciado automaticamente ficam em
`$XDG_STATE_HOME/padsmith/appimage-daemon.log` (ou
`~/.local/state/padsmith/appimage-daemon.log`).

No NixOS, execute pelo ambiente FHS do sistema:

```sh
appimage-run ./Padsmith_0.1.0_amd64.AppImage
appimage-run ./Padsmith_0.1.0_amd64.AppImage --cli status
```

Em sistemas sem FUSE, o runtime também permite `--appimage-extract-and-run`.
Não remova ou substitua o arquivo enquanto o daemon embutido estiver em uso.

## Permissões de controle

O AppImage não instala regras no sistema e roda como usuário comum. O acesso
a `/dev/uinput` e a todos os nós evdev do controle continua necessário.
No NixOS, use o módulo descrito em [nixos.md](nixos.md), ou acrescente à sua
configuração NixOS:

```nix
boot.kernelModules = [ "uinput" ];
services.udev.packages = [
  (pkgs.writeTextDir "lib/udev/rules.d/70-padsmith-uinput.rules" ''
    SUBSYSTEM=="misc", KERNEL=="uinput", OPTIONS+="static_node=uinput", TAG+="uaccess"
  '')
  (pkgs.writeTextDir "lib/udev/rules.d/71-padsmith-input.rules" ''
    SUBSYSTEM=="input", KERNEL=="event*", KERNELS=="000[356]:054C:09CC.*", TAG+="uaccess"
    SUBSYSTEM=="input", KERNEL=="event*", KERNELS=="000[356]:057E:2009.*", TAG+="uaccess"
  '')
];
```

O módulo deve receber `pkgs` nos argumentos. Aplique sua configuração e reconecte
o controle para atualizar as ACLs.

Em outras distribuições, as duas regras fornecidas podem ser instaladas pelo
administrador em `/etc/udev/rules.d/`. Carregue o módulo `uinput`, recarregue as
regras e reconecte o controle. O exemplo cobre os IDs `054c:09cc` e `057e:2009`;
acrescente seletores para outros modelos usando `--cli devices`.
As regras não ocultam hidraw; a configuração desse recurso permanece explícita
no módulo NixOS. Não é necessário executar a interface ou o daemon como root.
